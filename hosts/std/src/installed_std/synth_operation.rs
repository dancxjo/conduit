use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    CapabilityOffer, HostOperationRequirement, PlannedGear, CONTROL_EVENT_ENCODED_LEN,
    NOTE_EVENT_ENCODED_LEN,
};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) const SYNTH_HOST_OPERATION: &str = conduit_std_catalog::MUSIC_SYNTH_HOST_OPERATION;
pub(super) const PCM_BLOCK_BYTES: u32 = conduit_std_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES;

pub(super) struct InstalledSynthState {
    synth: conduit_synth::ReferenceSynth,
    last_event: Option<(u64, u32)>,
}

impl InstalledSynthState {
    pub(super) fn new() -> Result<Self, String> {
        Ok(Self {
            synth: conduit_synth::ReferenceSynth::new(
                conduit_synth::ReferenceSynthProfile::musician_reference(),
            )
            .map_err(|error| format!("prepare reference synth: {error:?}"))?,
            last_event: None,
        })
    }

    pub(super) fn stop(&mut self) {
        self.synth.stop();
    }
}

pub(super) fn execute(
    state: &mut InstalledSynthState,
    input: &[u8],
    output: &mut Vec<u8>,
) -> Result<bool, String> {
    enum Event {
        Note(conduit_core::MusicalNoteEvent),
        Control(conduit_core::MusicalControlEvent),
    }
    let event = if input.len() == NOTE_EVENT_ENCODED_LEN {
        Event::Note(
            conduit_core::MusicalNoteEvent::decode(input)
                .map_err(|error| format!("decode music note event: {error:?}"))?,
        )
    } else if input.len() == CONTROL_EVENT_ENCODED_LEN {
        Event::Control(
            conduit_core::MusicalControlEvent::decode(input)
                .map_err(|error| format!("decode music control event: {error:?}"))?,
        )
    } else {
        return Err("reference synth input has an unsupported exact length".to_string());
    };
    let key = match event {
        Event::Note(event) => (event.event_time_micros, event.order),
        Event::Control(event) => (event.event_time_micros, event.order),
    };
    if state.last_event.is_some_and(|last| key <= last) {
        return Err(
            "reference synth events are not in exact global timestamp/order sequence".to_string(),
        );
    }
    let target_frame = key
        .0
        .saturating_mul(u64::from(conduit_synth::REFERENCE_SAMPLE_RATE_HZ))
        / 1_000_000;
    let frames = target_frame
        .checked_sub(state.synth.frame_cursor())
        .ok_or_else(|| "reference synth event is stale".to_string())?;
    if frames > u64::from(conduit_synth::REFERENCE_MAXIMUM_BLOCK_FRAMES) {
        return Err("reference synth event exceeds the admitted block horizon".to_string());
    }
    output.clear();
    if frames > 0 {
        let frame_count = frames as u16;
        let header = conduit_core::PcmFrameHeader::new(
            conduit_core::PcmSampleRepresentation::Signed16LittleEndian,
            conduit_synth::REFERENCE_SAMPLE_RATE_HZ,
            conduit_core::PcmChannelLayout::Mono,
            frame_count,
            1,
            state.synth.frame_cursor(),
            false,
        )
        .map_err(|error| format!("frame reference synth PCM: {error:?}"))?;
        output.extend_from_slice(&header.encode());
        let payload_start = output.len();
        output.resize(payload_start + usize::from(frame_count) * 2, 0);
        let mut samples = [0_i16; conduit_synth::REFERENCE_MAXIMUM_BLOCK_FRAMES as usize];
        state.synth.render(&mut samples[..usize::from(frame_count)]);
        for (encoded, sample) in output[payload_start..]
            .chunks_exact_mut(2)
            .zip(samples.iter())
        {
            encoded.copy_from_slice(&sample.to_le_bytes());
        }
    }
    match event {
        Event::Note(event) => state.synth.apply_note(event),
        Event::Control(event) => state.synth.apply_control(event),
    }
    .map_err(|error| format!("apply reference synth event: {error:?}"))?;
    state.last_event = Some(key);
    Ok(frames > 0)
}

pub(super) static MUSIC_SYNTH_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_synth::REFERENCE_SYNTH_IMPLEMENTATION_ID,
    budget,
    prepare,
};

pub(super) fn host_requirement() -> HostOperationRequirement {
    conduit_std_catalog::music_synth_reference_offer().host_operations[0].clone()
}

pub(crate) fn offer() -> CapabilityOffer {
    conduit_std_catalog::music_synth_reference_offer()
}

pub(super) struct MusicSynthOperation {
    pending: Option<RequestId>,
    input: Option<conduit_kernel::ValueRef>,
    next_request: u32,
    closed: [bool; 2],
    completed: bool,
}

impl MusicSynthOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if self.pending.is_none()
                    && self.input.is_none()
                    && ((port == PortId(0) && value.byte_len == NOTE_EVENT_ENCODED_LEN as u32)
                        || (port == PortId(1)
                            && value.byte_len == CONTROL_EVENT_ENCODED_LEN as u32)) =>
            {
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.wrapping_add(1);
                self.pending = Some(request);
                self.input = Some(value);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(
                        value,
                        NOTE_EVENT_ENCODED_LEN.max(CONTROL_EVENT_ENCODED_LEN) as u32,
                    ) {
                        Ok(input) => input,
                        Err(_) => return InstalledOperation::fail(40),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.input = None;
                match outcome.output {
                    Some(output)
                        if output.admitted_bytes == PCM_BLOCK_BYTES
                            && output.value.byte_len <= PCM_BLOCK_BYTES =>
                    {
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    None => self.finish_or_await(),
                    _ => InstalledOperation::fail(41),
                }
            }
            OperationInput::Closed { port } if self.pending.is_none() && self.input.is_none() => {
                let index = usize::from(port.0);
                if index >= self.closed.len() || self.closed[index] {
                    return InstalledOperation::fail(42);
                }
                self.closed[index] = true;
                self.finish_or_await()
            }
            _ => InstalledOperation::fail(43),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.finish_or_await()
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.input = None;
        self.completed = true;
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        false
    }

    pub(super) fn take_released_value(&mut self) -> Option<conduit_kernel::ValueRef> {
        None
    }

    fn finish_or_await(&mut self) -> OperationAction {
        if self.closed == [true; 2] {
            self.completed = true;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 3,
        value_bytes: PCM_BLOCK_BYTES * 3,
        host_requests: 1,
        sign_items: 256,
        maximum_value_bytes: PCM_BLOCK_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    budget(placement)?;
    Ok(InstalledOperation::MusicSynth(MusicSynthOperation {
        pending: None,
        input: None,
        next_request: 0,
        closed: [false; 2],
        completed: false,
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != conduit_std_catalog::MUSIC_SYNTH_KIND
        || placement.kind_contract_revision.as_str() != conduit_std_catalog::MUSIC_SYNTH_REVISION
        || placement.execution_profile_id.as_str() != conduit_synth::REFERENCE_SYNTH_PROFILE_ID
        || placement.implementation_id.as_str() != conduit_synth::REFERENCE_SYNTH_IMPLEMENTATION_ID
        || placement.artifact_id.as_str() != conduit_synth::REFERENCE_SYNTH_ARTIFACT_ID
        || placement.host_operations != [host_requirement()]
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err(
            "planned music/synth placement does not match the installed reference profile"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, PcmFrameHeader};

    fn note(occurrence: u64, gate: Gate, micros: u64, order: u32) -> MusicalNoteEvent {
        MusicalNoteEvent::new(
            NoteOccurrenceId(occurrence),
            MusicalPitch::new(440_000, 440_000, 0).unwrap(),
            gate,
            u16::MAX,
            micros,
            order,
        )
        .unwrap()
    }

    #[test]
    fn admitted_event_interval_becomes_exact_bounded_pcm() {
        let mut state = InstalledSynthState::new().unwrap();
        let mut output = Vec::with_capacity(PCM_BLOCK_BYTES as usize);
        assert!(!execute(&mut state, &note(1, Gate::On, 0, 0).encode(), &mut output).unwrap());
        assert!(execute(
            &mut state,
            &note(1, Gate::Off, 5_000, 1).encode(),
            &mut output
        )
        .unwrap());
        let (header, payload) = PcmFrameHeader::decode_frame(&output).unwrap();
        assert_eq!(header.frame_count, 240);
        assert_eq!(header.start_frame, 0);
        assert_eq!(payload.len(), 480);
        assert!(payload.iter().any(|byte| *byte != 0));
        assert!(output.len() <= PCM_BLOCK_BYTES as usize);
    }

    #[test]
    fn global_order_and_block_horizon_are_refused_exactly() {
        let mut state = InstalledSynthState::new().unwrap();
        let mut output = Vec::with_capacity(PCM_BLOCK_BYTES as usize);
        execute(&mut state, &note(1, Gate::On, 0, 2).encode(), &mut output).unwrap();
        assert!(
            execute(&mut state, &note(1, Gate::Off, 0, 1).encode(), &mut output)
                .unwrap_err()
                .contains("global timestamp/order")
        );

        let mut later = InstalledSynthState::new().unwrap();
        assert!(execute(
            &mut later,
            &note(2, Gate::On, 1_000_000, 0).encode(),
            &mut output
        )
        .unwrap_err()
        .contains("block horizon"));
    }
}
