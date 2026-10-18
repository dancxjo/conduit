//! Transactional PCM traversal of one frozen admitted source projection.
use crate::{
    carried_formant::{FrozenCarriedFormant, PreparedOfflinePcm},
    kernel::{MAXIMUM_PCM_BYTES, REQUIRED_STEP_FUEL},
    render::RenderCursor,
    MAXIMUM_BLOCK_FRAMES,
};
use alloc::sync::Arc;
use conduit_audio::{
    PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, PCM_FRAME_HEADER_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};

/// The input is the exact 32-byte source identity. It is held until the final
/// PCM transaction; a different carrier cannot replace the prepared source.
pub struct CarriedFormantBack {
    source: Arc<PreparedOfflinePcm>,
    clock: u64,
    cursor: RenderCursor,
    staged: Option<RenderCursor>,
    input: Option<ValueRef>,
    staged_input: Option<ValueRef>,
    pcm: [u8; MAXIMUM_PCM_BYTES],
    pcm_len: usize,
    cancelled: bool,
}
impl CarriedFormantBack {
    pub fn prepare(source: Arc<PreparedOfflinePcm>, clock: u64) -> Result<Self, &'static str> {
        if clock == 0 {
            return Err("zero sample clock identity");
        }
        Ok(Self {
            cursor: source.source().render.initial(),
            source,
            clock,
            staged: None,
            input: None,
            staged_input: None,
            pcm: [0; MAXIMUM_PCM_BYTES],
            pcm_len: 0,
            cancelled: false,
        })
    }
    pub fn source(&self) -> &FrozenCarriedFormant {
        self.source.source()
    }
    pub fn rendered_frames(&self) -> u64 {
        self.cursor.rendered_frames()
    }
    fn propose<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        bytes: &StepInputBytes<'_, PORTS>,
    ) -> Result<StepOutcome, FailureCode> {
        self.staged = None;
        self.staged_input = None;
        self.pcm_len = 0;
        if self.cancelled {
            return Err(FailureCode::InvalidLifecycle);
        }
        let Some(input) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                Err(FailureCode::InvalidInput)
            } else {
                Ok(StepOutcome::Await)
            };
        };
        if self.input.is_some_and(|original| original != input) {
            return Err(FailureCode::InvalidLifecycle);
        }
        if input.byte_len != 32
            || bytes.input(PortId(0)) != Some(self.source.source().source_identity().as_slice())
        {
            return Err(FailureCode::InvalidInput);
        }
        if !io.output_ready(PortId(0)) {
            return Ok(StepOutcome::Await);
        }
        io.consume_fuel(REQUIRED_STEP_FUEL - 2)
            .map_err(|_| FailureCode::WorkBudgetExhausted)?;
        let mut next = self.cursor;
        let start = next.rendered_frames();
        let mut samples = [0_i16; 8];
        let mut count = 0;
        while count < MAXIMUM_BLOCK_FRAMES && !next.is_complete() {
            let capacity = samples.len().min(MAXIMUM_BLOCK_FRAMES - count);
            let rendered = self
                .source
                .source()
                .render
                .render(&mut next, &mut samples[..capacity])
                .map_err(|_| FailureCode::InvalidInput)?;
            if rendered == 0 {
                return Err(FailureCode::InvalidInput);
            }
            let offset = PCM_FRAME_HEADER_ENCODED_LEN + count * 2;
            for (sample, pair) in samples[..rendered].iter().zip(
                self.pcm[offset..offset + rendered * 2]
                    .as_chunks_mut::<2>()
                    .0
                    .iter_mut(),
            ) {
                pair.copy_from_slice(&sample.to_le_bytes());
            }
            count += rendered;
        }
        if count != 0 {
            let header = PcmFrameHeader::new(
                PcmSampleRepresentation::Signed16LittleEndian,
                crate::SAMPLE_RATE_HZ,
                PcmChannelLayout::Mono,
                count as u16,
                self.clock,
                start,
                false,
            )
            .map_err(|_| FailureCode::InvalidInput)?
            .encode();
            self.pcm[..header.len()].copy_from_slice(&header);
            self.pcm_len = header.len() + count * 2;
            io.send_prepared(PortId(0), self.pcm_len as u32)
                .map_err(|_| FailureCode::InvalidPort)?;
        }
        let outcome = if next.is_complete() {
            io.consume(PortId(0))
                .map_err(|_| FailureCode::InvalidPort)?;
            StepOutcome::Complete
        } else {
            StepOutcome::Progress
        };
        self.staged = Some(next);
        self.staged_input = Some(input);
        Ok(outcome)
    }
}
impl<const PORTS: usize> StepBack<PORTS> for CarriedFormantBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.propose(io, bytes)
            .unwrap_or_else(|code| StepOutcome::Fail(Failure { code, detail: 0 }))
    }
    fn step_committed(&mut self) {
        if let Some(cursor) = self.staged.take() {
            self.cursor = cursor;
            self.input = self.staged_input.take();
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.pcm_len != 0 && !self.cancelled)
            .then_some(&self.pcm[..self.pcm_len])
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = None;
        self.staged_input = None;
        self.pcm_len = 0;
    }
}
