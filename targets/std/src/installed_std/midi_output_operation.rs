use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) static MIDI_OUTPUT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::MUSIC_PLAY_MIDI_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct MidiOutputOperation {
    pending: Option<RequestId>,
    next_request: u32,
    closed: [bool; 2],
}

impl MidiOutputOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if self.pending.is_none()
                    && usize::from(port.0) < self.closed.len()
                    && !self.closed[usize::from(port.0)]
                    && self.next_request
                        < u32::from(conduit_semantic_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS) =>
            {
                let maximum = if port == PortId(0) {
                    conduit_audio::NOTE_EVENT_ENCODED_LEN as u32
                } else {
                    conduit_audio::CONTROL_EVENT_ENCODED_LEN as u32
                };
                let Ok(input) = BoundedValueRef::new(value, maximum) else {
                    return InstalledOperation::fail(82);
                };
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: if port == PortId(0) {
                        HostOperationId(1)
                    } else {
                        HostOperationId(0)
                    },
                    input,
                }
            }
            OperationInput::Closed { port }
                if self.pending.is_none() && usize::from(port.0) < self.closed.len() =>
            {
                self.closed[usize::from(port.0)] = true;
                if self.closed.into_iter().all(|closed| closed) {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                if let Some(failure) = outcome.failure {
                    return OperationAction::Fail(failure);
                }
                if outcome.disposition != HostOperationDisposition::Completed
                    || outcome.output.is_some()
                {
                    return InstalledOperation::fail(83);
                }
                OperationAction::Await
            }
            _ => InstalledOperation::fail(81),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.closed = [true; 2];
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: usize::from(conduit_semantic_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS),
        sign_items: 64,
        maximum_value_bytes: conduit_audio::NOTE_EVENT_ENCODED_LEN
            .max(conduit_audio::CONTROL_EVENT_ENCODED_LEN) as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::MidiOutput(MidiOutputOperation {
        pending: None,
        next_request: 0,
        closed: [false; 2],
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::music_play_midi_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || placement.inputs.len() != 2
        || placement.inputs[0].port_id.as_str() != "notes"
        || placement.inputs[1].port_id.as_str() != "controls"
        || placement
            .inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_std_offers::MIDI_OUTPUT_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || placement.authority.len() != 2
        || !placement.authority.iter().all(|authority| {
            authority.contract_id.as_str() == conduit_std_offers::MIDI_OUTPUT_AUTHORITY_CONTRACT
                && authority.host_id == placement.host_id
                && authority.boot_id == placement.boot_id
                && authority.capability_id == placement.capability_id
        })
        || !placement.configuration.is_empty()
    {
        return Err("planned music/play MIDI identity/resource/authority mismatch".into());
    }
    for (authority, operation) in placement.authority.iter().zip(&placement.host_operations) {
        if authority.host_operation_contract_id != operation.contract_id
            || authority.subject_kind.as_str()
                != operation
                    .target_kind
                    .as_ref()
                    .map_or("", conduit_core::KindId::as_str)
        {
            return Err("planned MIDI authority does not match typed operation".into());
        }
    }
    Ok(())
}

pub(super) fn prepare_session(
    placement: &PlannedGear,
    selected: Option<&crate::hosted_midi::MidiOutputSelection>,
) -> Result<crate::hosted_midi::MidiOutputSession, String> {
    validate(placement)?;
    let selected = selected
        .ok_or_else(|| "planned music/play has no exact selected MIDI output".to_string())?;
    let advertisement = selected
        .output_realization_advertisement(placement.host_id.clone())
        .map_err(str::to_string)?;
    if selected.boot_id() != &placement.boot_id
        || selected.offer_generation() != placement.offer_generation
        || placement.resources[0].pool_id != selected.resource_pool_id()
        || placement.realization_characteristics != advertisement.characteristics
    {
        return Err("planned MIDI output resource is stale or differs from selection".into());
    }
    crate::hosted_midi::MidiOutputSession::prepare(selected.clone())
        .map_err(|error| format!("open planned MIDI output: {error:?}"))
}

pub(super) fn prepare_adapter() -> Result<conduit_midi::MidiOutputAdapter, String> {
    let profile = conduit_midi::MidiProfile::new(
        crate::hosted_midi::A4_REFERENCE_MILLIHERTZ,
        None,
        crate::hosted_midi::OUTPUT_CHANNEL,
    )
    .map_err(|error| format!("prepare MIDI output profile: {error:?}"))?;
    Ok(conduit_midi::MidiOutputAdapter::new(profile))
}

pub(super) fn execute(
    adapter: &mut conduit_midi::MidiOutputAdapter,
    session: &mut crate::hosted_midi::MidiOutputSession,
    contract: &str,
    input: &[u8],
) -> conduit_kernel::HostOperationOutcome {
    if contract == conduit_std_offers::MUSIC_PLAY_MIDI_NOTE_OPERATION {
        let Ok(event) = conduit_audio::MusicalNoteEvent::decode(input) else {
            return failed(conduit_kernel::FailureCode::InvalidInput, 84);
        };
        let Ok(encoded) = adapter.encode_note(event) else {
            return failed(conduit_kernel::FailureCode::InvalidInput, 84);
        };
        return match session.send_note(event, encoded) {
            Ok(()) => completed(),
            Err(error) => output_failure(error),
        };
    }
    let encoded = if contract == conduit_std_offers::MUSIC_PLAY_MIDI_CONTROL_OPERATION {
        conduit_audio::MusicalControlEvent::decode(input)
            .map_err(|_| ())
            .and_then(|event| adapter.encode_control(event).map_err(|_| ()))
    } else {
        Err(())
    };
    match encoded {
        Ok(encoded) => match session.send(encoded) {
            Ok(()) => completed(),
            Err(error) => output_failure(error),
        },
        Err(()) => failed(conduit_kernel::FailureCode::InvalidInput, 84),
    }
}

fn completed() -> conduit_kernel::HostOperationOutcome {
    conduit_kernel::HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: None,
        failure: None,
    }
}

fn output_failure(
    error: crate::hosted_midi::MidiOutputFailure,
) -> conduit_kernel::HostOperationOutcome {
    use crate::hosted_midi::MidiOutputFailure;
    match error {
        MidiOutputFailure::BackendUnavailable => denied(85),
        MidiOutputFailure::Pressure => failed(conduit_kernel::FailureCode::StorageExhausted, 86),
        MidiOutputFailure::ProviderLost => {
            failed(conduit_kernel::FailureCode::HostOperationFailed, 87)
        }
        MidiOutputFailure::InvalidLifecycle => {
            failed(conduit_kernel::FailureCode::InvalidLifecycle, 88)
        }
    }
}

fn denied(detail: u16) -> conduit_kernel::HostOperationOutcome {
    conduit_kernel::HostOperationOutcome {
        disposition: HostOperationDisposition::Denied,
        output: None,
        failure: Some(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::HostOperationDenied,
            detail,
        }),
    }
}

fn failed(code: conduit_kernel::FailureCode, detail: u16) -> conduit_kernel::HostOperationOutcome {
    conduit_kernel::HostOperationOutcome {
        disposition: HostOperationDisposition::Failed,
        output: None,
        failure: Some(conduit_kernel::Failure { code, detail }),
    }
}

#[cfg(test)]
#[path = "midi_output_operation_tests.rs"]
mod tests;
