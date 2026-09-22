use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{ConfigurationValue, PlannedGear, PortDirection};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostCallDisposition, HostCallId,
    HostCallOutcome, PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_midi::{
    MidiInputAdapter, MidiInputObservation, MidiMessage, MidiProfile, ParsedMidi, PortableMidiEvent,
};

pub(super) static MIDI_INPUT_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::MUSIC_INPUT_MIDI_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct MidiInputBack {
    adapter: MidiInputAdapter,
    empty_input: ValueRef,
    pending: Option<RequestId>,
    next_request: u32,
    emitted: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for MidiInputBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
                || outcome.output.is_none()
            {
                return outcome
                    .failure
                    .map_or_else(|| step_fail(92), StepOutcome::Fail);
            }
            let Some(canonical) = input_bytes.host_output() else {
                return step_fail(93);
            };
            let Ok(observation) = MidiInputObservation::decode(canonical) else {
                return step_failure(FailureCode::InvalidInput, 94);
            };
            let ParsedMidi::Message(message) = observation.parsed else {
                return step_failure(FailureCode::InvalidInput, 95);
            };
            let port = match message {
                MidiMessage::NoteOn { .. } | MidiMessage::NoteOff { .. } => PortId(0),
                MidiMessage::ControlChange { .. } | MidiMessage::PitchBend { .. } => PortId(1),
                _ => return step_failure(FailureCode::InvalidInput, 96),
            };
            if !io.output_ready(port) {
                return StepOutcome::Await;
            }
            let event = match self.adapter.accept(message, observation.event_time_micros) {
                Ok(PortableMidiEvent::Note(event)) => CanonicalValue::new(&event.encode()),
                Ok(PortableMidiEvent::Control(event)) => CanonicalValue::new(&event.encode()),
                Ok(PortableMidiEvent::UnsupportedControl { .. })
                | Ok(PortableMidiEvent::IgnoredChannel { .. }) => {
                    return step_failure(FailureCode::InvalidInput, 96);
                }
                Err(_) => return step_failure(FailureCode::InvalidInput, 97),
            };
            let Ok(value) = event else {
                return step_failure(FailureCode::StorageExhausted, 98);
            };
            io.consume_host_completion()
                .expect("observed MIDI input completion");
            io.send_canonical(port, value)
                .expect("ready MIDI event output");
            self.pending = None;
            self.emitted = true;
            return StepOutcome::Progress;
        }

        if self.pending.is_none() {
            if self.next_request >= u32::from(conduit_semantic_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS)
            {
                return step_failure(FailureCode::StorageExhausted, 101);
            }
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_failure(FailureCode::StorageExhausted, 101);
            };
            let input = BoundedValueRef::new(self.empty_input, 0)
                .expect("empty MIDI source request is exact");
            io.request_host_call(request, HostCallId(0), input)
                .expect("MIDI input Host Call");
            self.next_request = next;
            self.pending = Some(request);
            self.emitted = false;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.adapter.cancel();
        self.pending = None;
        self.emitted = false;
    }
}

const fn step_failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

const fn step_fail(detail: u16) -> StepOutcome {
    step_failure(FailureCode::InvalidLifecycle, detail)
}

impl MidiInputBack {}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    profile(placement)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: (conduit_midi::MIDI_INPUT_OBSERVATION_ENCODED_LEN
            + conduit_audio::NOTE_EVENT_ENCODED_LEN) as u32,
        host_requests: usize::from(conduit_semantic_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS),
        sign_items: conduit_semantic_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS.saturating_mul(4),
        maximum_value_bytes: conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    let profile = profile(placement)?;
    let adapter = MidiInputAdapter::new(profile, 1)
        .map_err(|error| format!("prepare MIDI input occurrence sequence: {error:?}"))?;
    let empty_input = values
        .store(&[])
        .map_err(|error| format!("reserve empty MIDI input request: {error:?}"))?;
    Ok(InstalledBack::MidiInput(Box::new(MidiInputBack {
        adapter,
        empty_input,
        pending: None,
        next_request: 0,
        emitted: false,
    })))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::music_input_midi_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || !placement.inputs.is_empty()
        || placement.outputs.len() != 2
        || placement.outputs[0].port_id.as_str() != "notes"
        || placement.outputs[1].port_id.as_str() != "controls"
        || placement
            .outputs
            .iter()
            .any(|port| port.direction != PortDirection::Output)
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str() != conduit_std_offers::MIDI_INPUT_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || placement.authority.len() != 1
        || !configuration_is_exact(placement)
    {
        return Err("planned music/input MIDI identity/resource/authority mismatch".into());
    }
    let authority = &placement.authority[0];
    let operation = &placement.host_calls[0];
    if authority.contract_id.as_str() != conduit_std_offers::MIDI_INPUT_AUTHORITY_CONTRACT
        || authority.host_call_contract_id != operation.contract_id
        || authority.subject_kind.as_str()
            != operation
                .target_kind
                .as_ref()
                .map_or("", conduit_core::KindId::as_str)
        || authority.host_id != placement.host_id
        || authority.boot_id != placement.boot_id
        || authority.capability_id != placement.capability_id
    {
        return Err("planned MIDI input authority does not match typed operation".into());
    }
    Ok(())
}

fn configuration_is_exact(placement: &PlannedGear) -> bool {
    let expected = conduit_semantic_catalog::music_input_configuration();
    placement.configuration.len() == expected.len()
        && expected.iter().all(|field| {
            placement
                .configuration
                .iter()
                .filter(|entry| entry.key == field.key)
                .count()
                == 1
        })
}

fn profile(placement: &PlannedGear) -> Result<MidiProfile, String> {
    let a4_reference_millihertz = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (
                conduit_semantic_catalog::MUSIC_INPUT_A4_REFERENCE_KEY,
                ConfigurationValue::U64(value),
            ) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| "planned MIDI input A4 reference is missing or invalid".to_string())?;
    let transpose_semitones = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (
                conduit_semantic_catalog::MUSIC_INPUT_TRANSPOSE_KEY,
                ConfigurationValue::I64(value),
            ) => i16::try_from(*value).ok(),
            _ => None,
        })
        .ok_or_else(|| "planned MIDI input transpose is missing or invalid".to_string())?;
    MidiProfile::new(a4_reference_millihertz, None, 0)
        .and_then(|profile| profile.with_transpose(transpose_semitones))
        .map_err(|error| format!("prepare planned MIDI input profile: {error:?}"))
}

pub(super) fn prepare_session(
    placement: &PlannedGear,
    selected: Option<&crate::hosted_midi::HostedRawMidiSelection>,
) -> Result<crate::hosted_midi::MidiInputSession, String> {
    validate(placement)?;
    let selected = selected
        .ok_or_else(|| "planned music/input has no exact selected MIDI input".to_string())?;
    let advertisement = selected
        .input_realization_advertisement(placement.host_id.clone())
        .map_err(str::to_string)?;
    if selected.boot_id() != &placement.boot_id
        || selected.offer_generation() != placement.offer_generation
        || placement.resources[0].pool_id != selected.resource_pool_id()
        || placement.realization_characteristics != advertisement.characteristics
    {
        return Err("planned MIDI input resource is stale or differs from selection".into());
    }
    crate::hosted_midi::MidiInputSession::prepare(selected)
        .map_err(|error| format!("open planned MIDI input: {error:?}"))
}

pub(super) fn failure_outcome(error: crate::hosted_midi::MidiInputFailure) -> HostCallOutcome {
    use crate::hosted_midi::MidiInputFailure;
    let (disposition, code, detail) = match error {
        MidiInputFailure::BackendUnavailable => (
            HostCallDisposition::Denied,
            FailureCode::HostCallDenied,
            102,
        ),
        MidiInputFailure::ProviderLost => (
            HostCallDisposition::Failed,
            FailureCode::HostCallFailed,
            103,
        ),
        MidiInputFailure::Malformed(_) => {
            (HostCallDisposition::Failed, FailureCode::InvalidInput, 104)
        }
        MidiInputFailure::CapacityExceeded => (
            HostCallDisposition::Failed,
            FailureCode::StorageExhausted,
            105,
        ),
        MidiInputFailure::ClockRegressed => (
            HostCallDisposition::Failed,
            FailureCode::InvalidLifecycle,
            106,
        ),
        MidiInputFailure::InvalidLifecycle => (
            HostCallDisposition::Failed,
            FailureCode::InvalidLifecycle,
            107,
        ),
    };
    HostCallOutcome {
        disposition,
        output: None,
        failure: Some(Failure { code, detail }),
    }
}

#[cfg(test)]
#[path = "midi_input_back_tests.rs"]
mod tests;
