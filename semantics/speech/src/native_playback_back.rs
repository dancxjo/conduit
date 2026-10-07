//! Prepared native tape in the existing transactional kernel. No text parsing,
//! language policy, timer, scheduling, device effect or Play allocation.
use crate::{
    native_playback_contract as contract, playback_basis::PreparedSpeechPlaybackTape, semantic::*,
    Renderer,
};
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    FailureCode, PortId, ValueRef,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePlaybackPreparationRefusal {
    Identity,
    Configuration,
    Port,
    Renderer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackAcknowledgementRefusal {
    Basis,
    Range,
    NotQueued,
    Contiguity,
}
/// Basis/range validation from this queued producer; device authority remains
/// with the effect owner that supplied the native acknowledgement.
#[derive(Clone, Copy)]
pub struct PlayedTapeEvidence<'t, 'e> {
    tape: &'t PreparedSpeechPlaybackTape<'t>,
    acknowledgement: &'e SpeechPlaybackAcknowledgement,
}
impl<'t, 'e> PlayedTapeEvidence<'t, 'e> {
    pub fn tape(&self) -> &'t PreparedSpeechPlaybackTape<'t> {
        self.tape
    }
    pub fn acknowledgement(&self) -> &'e SpeechPlaybackAcknowledgement {
        self.acknowledgement
    }
}
pub struct NativeSpeechPlaybackBack<'a> {
    tape: &'a PreparedSpeechPlaybackTape<'a>,
    cursor: Renderer<'a>,
    staged: Option<Renderer<'a>>,
    input: Option<ValueRef>,
    pending_input: Option<ValueRef>,
    pcm: [u8; contract::MAXIMUM_PCM_BYTES],
    pcm_len: usize,
    input_port: PortId,
    output_port: PortId,
    acknowledged_queued: u64,
    played_through: u64,
    cancelled: bool,
}
impl<'a> NativeSpeechPlaybackBack<'a> {
    pub fn prepare<const PORTS: usize>(
        placement: &PlannedGear,
        tape: &'a PreparedSpeechPlaybackTape<'a>,
        input_port: PortId,
        output_port: PortId,
    ) -> Result<Self, NativePlaybackPreparationRefusal> {
        let expected =
            contract::offer(tape).map_err(|_| NativePlaybackPreparationRefusal::Configuration)?;
        let kind = contract::contract(tape)
            .map_err(|_| NativePlaybackPreparationRefusal::Configuration)?;
        if placement.kind_id != expected.kind_id
            || placement.kind_contract_revision != expected.kind_contract_revision
            || placement.execution_profile_id != expected.implementation.execution_profile_id
            || placement.capability_id != expected.capability_id
            || placement.implementation_id != expected.implementation.implementation_id
            || placement.artifact_id != expected.implementation.artifact_id
            || placement.realization_properties != expected.realization_properties
            || placement.inputs != expected.inputs
            || placement.outputs != expected.outputs
            || placement.limits != expected.limits
            || placement.semantic_contract != kind.semantic_contract()
            || !placement.host_calls.is_empty()
            || !placement.resources.is_empty()
            || !placement.authority.is_empty()
            || placement.base.is_some()
            || !placement.realization_characteristics.is_empty()
            || !placement.pool_references.is_empty()
            || !placement.terminal_transductions.is_empty()
        {
            return Err(NativePlaybackPreparationRefusal::Identity);
        }
        if PORTS == 0 || input_port != PortId(0) || output_port != PortId(0) {
            return Err(NativePlaybackPreparationRefusal::Port);
        }
        let [entry] = placement.configuration.as_slice() else {
            return Err(NativePlaybackPreparationRefusal::Configuration);
        };
        let ConfigurationValue::Structured(basis) = &entry.value else {
            return Err(NativePlaybackPreparationRefusal::Configuration);
        };
        if entry.key != "basis"
            || basis.profile() != tape.profile()
            || basis.canonical_value() != tape.canonical()
        {
            return Err(NativePlaybackPreparationRefusal::Configuration);
        }
        let cursor = tape
            .renderer()
            .map_err(|_| NativePlaybackPreparationRefusal::Renderer)?;
        Ok(Self {
            tape,
            cursor,
            staged: None,
            input: None,
            pending_input: None,
            pcm: [0; contract::MAXIMUM_PCM_BYTES],
            pcm_len: 0,
            input_port,
            output_port,
            acknowledged_queued: 0,
            played_through: 0,
            cancelled: false,
        })
    }
    pub fn tape(&self) -> &'a PreparedSpeechPlaybackTape<'a> {
        self.tape
    }
    pub fn queued_frames(&self) -> u64 {
        self.cursor.rendered_frames()
    }
    pub fn played_frames(&self) -> u64 {
        self.played_through
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    /// Validate an explicit effect-owner assertion. This is not device/authority
    /// attestation. Cancel cannot erase later evidence of already queued audio.
    pub fn acknowledge<'e>(
        &mut self,
        ack: &'e SpeechPlaybackAcknowledgement,
    ) -> Result<Option<PlayedTapeEvidence<'a, 'e>>, PlaybackAcknowledgementRefusal> {
        if ack.basis() != self.tape.basis() {
            return Err(PlaybackAcknowledgementRefusal::Basis);
        }
        let first = *ack.first_frame();
        let end = *ack.through_frame();
        if end <= first {
            return Err(PlaybackAcknowledgementRefusal::Range);
        }
        if end > self.queued_frames() {
            return Err(PlaybackAcknowledgementRefusal::NotQueued);
        }
        let frontier = match ack.disposition() {
            SpeechPlaybackDisposition::Queued => &mut self.acknowledged_queued,
            SpeechPlaybackDisposition::Played => &mut self.played_through,
        };
        if first != *frontier {
            return Err(PlaybackAcknowledgementRefusal::Contiguity);
        }
        *frontier = end;
        Ok(match ack.disposition() {
            SpeechPlaybackDisposition::Queued => None,
            SpeechPlaybackDisposition::Played => Some(PlayedTapeEvidence {
                tape: self.tape,
                acknowledgement: ack,
            }),
        })
    }
    fn propose<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        bytes: &StepInputBytes<'_, PORTS>,
    ) -> Result<StepOutcome, FailureCode> {
        self.staged = None;
        self.pending_input = None;
        self.pcm_len = 0;
        if self.cancelled {
            return Ok(StepOutcome::Complete);
        }
        let Some(reference) = io.input(self.input_port) else {
            return if io.input_closed(self.input_port) {
                io.consume_closed(self.input_port)
                    .map_err(|_| FailureCode::InvalidPort)?;
                Ok(StepOutcome::Complete)
            } else {
                Ok(StepOutcome::Await)
            };
        };
        let raw = bytes
            .input(self.input_port)
            .ok_or(FailureCode::InvalidInput)?;
        if raw != 0_u64.to_le_bytes() || self.input.is_some_and(|old| old != reference) {
            return Err(FailureCode::InvalidInput);
        }
        if !io.output_ready(self.output_port) {
            return Ok(StepOutcome::Await);
        }
        io.consume_fuel(contract::REQUIRED_STEP_FUEL - 2)
            .map_err(|_| FailureCode::WorkBudgetExhausted)?;
        let mut next = self.cursor;
        let mut pcm = [0_i16; crate::MAXIMUM_BLOCK_FRAMES];
        let first = next.rendered_frames();
        let count = next
            .render(&mut pcm)
            .map_err(|_| FailureCode::InvalidInput)?;
        if count != 0 {
            let header = PcmFrameHeader::new(
                PcmSampleRepresentation::Signed16LittleEndian,
                crate::SAMPLE_RATE_HZ,
                PcmChannelLayout::Mono,
                count as u16,
                *self.tape.basis().clock_id(),
                first,
                false,
            )
            .map_err(|_| FailureCode::InvalidInput)?
            .encode();
            self.pcm[..header.len()].copy_from_slice(&header);
            for (sample, pair) in pcm[..count].iter().zip(
                self.pcm[header.len()..header.len() + count * 2]
                    .as_chunks_mut::<2>()
                    .0
                    .iter_mut(),
            ) {
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
impl<const PORTS: usize> StepBack<PORTS> for NativeSpeechPlaybackBack<'_> {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self.propose(io, bytes) {
            Ok(outcome) => outcome,
            Err(code) => StepOutcome::Fail(conduit_kernel::Failure { code, detail: 0 }),
        }
    }
    fn step_committed(&mut self) {
        if let Some(next) = self.staged.take() {
            self.cursor = next;
            self.input = self.pending_input.take();
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == self.output_port && self.pcm_len != 0).then_some(&self.pcm[..self.pcm_len])
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = None;
        self.pending_input = None;
        self.pcm_len = 0;
    }
}
