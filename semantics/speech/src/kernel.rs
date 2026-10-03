//! Fixed-storage compiled speech Back for the existing execution kernel.
//! Preparation allocates contract descriptions; a Step uses only admitted arrays.
mod contract;
use crate::{
    pronounce, render::RenderCursor, TextRefusal, VoiceBoundary, VoiceEvent, MAXIMUM_BLOCK_FRAMES,
    MAXIMUM_EVENTS, MAXIMUM_TEXT_BYTES,
};
use conduit_audio::{
    PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, PCM_FRAME_HEADER_ENCODED_LEN,
};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};
pub use contract::{contract, install, offer, CAPABILITY, IMPLEMENTATION, KIND, PROFILE, REVISION};

pub const MAXIMUM_PCM_BYTES: usize = PCM_FRAME_HEADER_ENCODED_LEN + 2 * MAXIMUM_BLOCK_FRAMES;
/// One finite computation quantum (at most 512 text bytes, 256 events,
/// and 128 frames), plus output and final-input transactional actions.
/// Fuel counts cooperative quanta, not CPU cycles or hard containment.
pub const REQUIRED_STEP_FUEL: u16 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationRefusal {
    Identity,
    Configuration,
    Port,
}

/// Stable details under `FailureCode::InvalidInput`; unsupported text is
/// distinguishable from malformed encoding, admission bounds and DSP refusal.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFailureDetail {
    InvalidUtf8 = 1,
    TextBound = 2,
    WordBound = 3,
    EventBound = 4,
    EventStorage = 5,
    UnsupportedCharacter = 6,
    Arithmetic = 7,
}
struct BackFailure(Failure);
impl From<FailureCode> for BackFailure {
    fn from(code: FailureCode) -> Self {
        Self(Failure { code, detail: 0 })
    }
}
impl From<TextRefusal> for BackFailure {
    fn from(value: TextRefusal) -> Self {
        let detail = match value {
            TextRefusal::InputBound => InputFailureDetail::TextBound,
            TextRefusal::WordBound => InputFailureDetail::WordBound,
            TextRefusal::EventBound => InputFailureDetail::EventBound,
            TextRefusal::OutputSpace => InputFailureDetail::EventStorage,
            TextRefusal::UnsupportedCharacter { .. } => InputFailureDetail::UnsupportedCharacter,
            TextRefusal::Arithmetic => InputFailureDetail::Arithmetic,
        };
        input_failure(detail)
    }
}
fn input_failure(detail: InputFailureDetail) -> BackFailure {
    BackFailure(Failure {
        code: FailureCode::InvalidInput,
        detail: detail as u16,
    })
}

pub struct NativeSpeechBack {
    events: [VoiceEvent; MAXIMUM_EVENTS],
    event_count: usize,
    cursor: Option<RenderCursor>,
    staged: Option<RenderCursor>,
    input: Option<ValueRef>,
    pending_input: Option<ValueRef>,
    pcm: [u8; MAXIMUM_PCM_BYTES],
    pcm_len: usize,
    clock: u64,
    input_port: PortId,
    output_port: PortId,
}
impl NativeSpeechBack {
    pub fn prepare<const PORTS: usize>(
        placement: &PlannedGear,
        input_port: PortId,
        output_port: PortId,
    ) -> Result<Self, PreparationRefusal> {
        let expected = offer();
        if placement.kind_id != expected.kind_id
            || placement.kind_contract_revision != expected.kind_contract_revision
            || placement.execution_profile_id != expected.implementation.execution_profile_id
            || placement.capability_id != expected.capability_id
            || placement.implementation_id != expected.implementation.implementation_id
            || placement.artifact_id != expected.implementation.artifact_id
            || placement.inputs != expected.inputs
            || placement.outputs != expected.outputs
            || placement.limits != expected.limits
            || placement.semantic_contract != contract().semantic_contract()
            || !placement.host_calls.is_empty()
            || !placement.resources.is_empty()
            || !placement.authority.is_empty()
            || placement.base.is_some()
            || !placement.realization_characteristics.is_empty()
            || !placement.pool_references.is_empty()
            || !placement.terminal_transductions.is_empty()
        {
            return Err(PreparationRefusal::Identity);
        }
        if PORTS == 0 || input_port != PortId(0) || output_port != PortId(0) {
            return Err(PreparationRefusal::Port);
        }
        let clock = match placement.configuration.as_slice() {
            [entry] if entry.key == "clock" => match entry.value {
                ConfigurationValue::U64(clock) if clock != 0 => clock,
                _ => return Err(PreparationRefusal::Configuration),
            },
            _ => return Err(PreparationRefusal::Configuration),
        };
        Ok(Self {
            events: [VoiceEvent::boundary(VoiceBoundary::phrase); MAXIMUM_EVENTS],
            event_count: 0,
            cursor: None,
            staged: None,
            input: None,
            pending_input: None,
            pcm: [0; MAXIMUM_PCM_BYTES],
            pcm_len: 0,
            clock,
            input_port,
            output_port,
        })
    }
    pub fn rendered_frames(&self) -> u64 {
        self.cursor.map(|c| c.rendered_frames()).unwrap_or(0)
    }
    fn propose<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        bytes: &StepInputBytes<'_, PORTS>,
    ) -> Result<StepOutcome, BackFailure> {
        self.staged = None;
        self.pending_input = None;
        self.pcm_len = 0;
        let Some(reference) = io.input(self.input_port) else {
            if io.input_closed(self.input_port) {
                return Err(FailureCode::InvalidInput.into());
            }
            return Ok(StepOutcome::Await);
        };
        if self.input.is_some_and(|exact| exact != reference) {
            return Err(FailureCode::InvalidLifecycle.into());
        }
        if !io.output_ready(self.output_port) {
            return Ok(StepOutcome::Await);
        }
        // Reserve all private work before beginning it. Nothing in this path allocates.
        io.consume_fuel(REQUIRED_STEP_FUEL - 2)
            .map_err(|_| FailureCode::WorkBudgetExhausted)?;
        let mut next = match self.cursor {
            Some(cursor) => cursor,
            None => {
                let raw = bytes
                    .input(self.input_port)
                    .ok_or(FailureCode::InvalidInput)?;
                if raw.len() != reference.byte_len as usize {
                    return Err(FailureCode::InvalidInput.into());
                }
                if raw.len() > MAXIMUM_TEXT_BYTES {
                    return Err(input_failure(InputFailureDetail::TextBound));
                }
                let text = core::str::from_utf8(raw)
                    .map_err(|_| input_failure(InputFailureDetail::InvalidUtf8))?;
                let prepared = pronounce(text, &mut self.events)?;
                self.event_count = prepared.events().len();
                RenderCursor::prepare(prepared.events())
                    .map_err(|_| input_failure(InputFailureDetail::Arithmetic))?
            }
        };
        let start = next.rendered_frames();
        let mut samples = [0_i16; MAXIMUM_BLOCK_FRAMES];
        let count = next
            .render(&self.events[..self.event_count], &mut samples)
            .map_err(|_| input_failure(InputFailureDetail::Arithmetic))?;
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
            for (sample, pair) in samples[..count]
                .iter()
                .zip(self.pcm[header.len()..].as_chunks_mut::<2>().0.iter_mut())
            {
                pair.copy_from_slice(&sample.to_le_bytes());
            }
            self.pcm_len = header.len() + count * 2;
            io.send_prepared(self.output_port, self.pcm_len as u32)
                .map_err(|_| FailureCode::InvalidPort)?;
        }
        let outcome = if next.is_complete() {
            io.consume(self.input_port)
                .map_err(|_| FailureCode::InvalidPort)?;
            StepOutcome::Complete
        } else {
            StepOutcome::Progress
        };
        self.staged = Some(next);
        self.pending_input = Some(reference);
        Ok(outcome)
    }
}
impl<const PORTS: usize> StepBack<PORTS> for NativeSpeechBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self.propose(io, bytes) {
            Ok(outcome) => outcome,
            Err(failure) => StepOutcome::Fail(failure.0),
        }
    }
    fn step_committed(&mut self) {
        if let Some(cursor) = self.staged.take() {
            self.cursor = Some(cursor);
            self.input = self.pending_input.take();
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == self.output_port && self.pcm_len != 0).then_some(&self.pcm[..self.pcm_len])
    }
    fn cancel(&mut self) {
        self.staged = None;
        self.pending_input = None;
        self.pcm_len = 0;
    }
}
