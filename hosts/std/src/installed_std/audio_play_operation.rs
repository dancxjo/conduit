use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{CapabilityOffer, PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, HostedValueStore, OperationAction,
    OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) const DRAIN_MARKER: [u8; 1] = [0xff];
pub(super) const HOST_OPERATION: &str = conduit_std_catalog::AUDIO_PLAY_ALSA_HW_OPERATION;

pub(super) static AUDIO_PLAY_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct AudioPlayOperation {
    pending: Option<RequestId>,
    next_request: u32,
    drain_marker: ValueRef,
    draining: bool,
    closed: bool,
}

impl AudioPlayOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none()
                && !self.closed
                && self.next_request
                    < u32::from(conduit_std_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS) =>
            {
                self.request(value, false)
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && !self.closed =>
            {
                self.closed = true;
                self.request(self.drain_marker, true)
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
                    return InstalledOperation::fail(61);
                }
                if self.draining {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => InstalledOperation::fail(60),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.closed = true;
    }

    fn request(&mut self, value: ValueRef, drain: bool) -> OperationAction {
        let request = RequestId(self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        self.pending = Some(request);
        self.draining = drain;
        let Ok(input) =
            BoundedValueRef::new(value, conduit_std_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES)
        else {
            return InstalledOperation::fail(62);
        };
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input,
        }
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: DRAIN_MARKER.len() as u32,
        host_requests: usize::from(conduit_std_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS) + 1,
        sign_items: 64,
        maximum_value_bytes: conduit_std_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let drain_marker = values
        .store(&DRAIN_MARKER)
        .map_err(|error| format!("store audio/play drain marker: {error:?}"))?;
    Ok(InstalledOperation::AudioPlay(AudioPlayOperation {
        pending: None,
        next_request: 0,
        drain_marker,
        draining: false,
        closed: false,
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    let resource = placement.resources.first();
    let authority = placement.authority.first();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || placement.inputs.len() != 1
        || placement.inputs[0].port_id.as_str() != "audio"
        || placement.inputs[0].direction != PortDirection::Input
        || placement.resources.len() != 1
        || resource.is_none_or(|binding| {
            binding.class_id.as_str() != conduit_std_catalog::AUDIO_PLAYBACK_RESOURCE_CLASS
                || binding.units != 1
                || binding.protected.is_some()
                || binding.compute.is_some()
        })
        || placement.authority.len() != 1
        || authority.is_none_or(|binding| {
            binding.contract_id.as_str() != conduit_std_catalog::AUDIO_PLAYBACK_AUTHORITY_CONTRACT
                || binding.host_operation_contract_id.as_str() != HOST_OPERATION
                || binding.subject_kind.as_str() != conduit_core::AUDIO_PCM_INFO_ID
                || binding.host_id != placement.host_id
                || binding.boot_id != placement.boot_id
                || binding.capability_id != placement.capability_id
        })
        || !placement.configuration.is_empty()
    {
        return Err(
            "planned audio/play identity/resource/authority does not match installation"
                .to_string(),
        );
    }
    Ok(())
}

pub(super) fn offer() -> CapabilityOffer {
    conduit_std_catalog::audio_play_alsa_hw_offer()
}

pub(super) fn prepare_session(
    placement: &PlannedGear,
    selected: Option<&crate::hosted_audio::HostedPlaybackSelection>,
) -> Result<crate::hosted_audio::PlaybackSession, String> {
    validate(placement)?;
    let selected = selected.ok_or_else(|| {
        "planned audio/play has no exact selected hosted playback resource".to_string()
    })?;
    if selected.boot_id != placement.boot_id
        || selected.offer_generation != placement.offer_generation
        || placement.resources[0].pool_id != selected.pool_id()
        || placement.realization_characteristics
            != selected
                .realization_advertisement(placement.host_id.clone())
                .characteristics
    {
        return Err("planned audio/play resource is stale or differs from selection".to_string());
    }
    Ok(crate::hosted_audio::PlaybackSession::resolved(
        selected.clone(),
    ))
}

pub(super) fn execute(
    session: &mut crate::hosted_audio::PlaybackSession,
    input: &[u8],
) -> conduit_kernel::HostOperationOutcome {
    let result = if input == DRAIN_MARKER {
        session.drain()
    } else {
        session.write_frame(input)
    };
    match result {
        Ok(()) => conduit_kernel::HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: None,
            failure: None,
        },
        Err(error) => failure_outcome(error),
    }
}

fn failure_outcome(
    error: crate::hosted_audio::PlaybackFailure,
) -> conduit_kernel::HostOperationOutcome {
    use crate::hosted_audio::PlaybackFailure;
    let (disposition, code, detail) = match error {
        PlaybackFailure::StaleObservation => (
            conduit_kernel::HostOperationDisposition::Denied,
            conduit_kernel::FailureCode::HostOperationDenied,
            70,
        ),
        PlaybackFailure::DeviceBusy => (
            conduit_kernel::HostOperationDisposition::Denied,
            conduit_kernel::FailureCode::HostOperationDenied,
            71,
        ),
        PlaybackFailure::OpenFailed => (
            conduit_kernel::HostOperationDisposition::Failed,
            conduit_kernel::FailureCode::HostOperationFailed,
            72,
        ),
        PlaybackFailure::InvalidPcm | PlaybackFailure::DiscontinuousInput => (
            conduit_kernel::HostOperationDisposition::Failed,
            conduit_kernel::FailureCode::InvalidInput,
            73,
        ),
        PlaybackFailure::Underrun => (
            conduit_kernel::HostOperationDisposition::Failed,
            conduit_kernel::FailureCode::HostOperationFailed,
            74,
        ),
        PlaybackFailure::ProviderLost => (
            conduit_kernel::HostOperationDisposition::Failed,
            conduit_kernel::FailureCode::HostOperationFailed,
            75,
        ),
        PlaybackFailure::WriteFailed => (
            conduit_kernel::HostOperationDisposition::Failed,
            conduit_kernel::FailureCode::HostOperationFailed,
            76,
        ),
        PlaybackFailure::DrainFailed => (
            conduit_kernel::HostOperationDisposition::Failed,
            conduit_kernel::FailureCode::HostOperationFailed,
            77,
        ),
        PlaybackFailure::CloseFailed => (
            conduit_kernel::HostOperationDisposition::Failed,
            conduit_kernel::FailureCode::HostOperationFailed,
            78,
        ),
        PlaybackFailure::InvalidLifecycle => (
            conduit_kernel::HostOperationDisposition::Failed,
            conduit_kernel::FailureCode::InvalidLifecycle,
            79,
        ),
    };
    conduit_kernel::HostOperationOutcome {
        disposition,
        output: None,
        failure: Some(conduit_kernel::Failure { code, detail }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostOperationOutcome;

    #[test]
    fn input_is_serialized_and_close_requests_exact_drain() {
        let mut operation = AudioPlayOperation {
            pending: None,
            next_request: 0,
            drain_marker: ValueRef {
                slot: 9,
                generation: 1,
                byte_len: 1,
            },
            draining: false,
            closed: false,
        };
        assert_eq!(operation.start(), OperationAction::Await);
        let value = ValueRef {
            slot: 2,
            generation: 1,
            byte_len: 100,
        };
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value
            }),
            OperationAction::RequestHostOperation { request: RequestId(0), input, .. }
                if input.value == value
        ));
        assert_eq!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            }),
            OperationAction::Await
        );
        assert!(matches!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::RequestHostOperation { request: RequestId(1), input, .. }
                if input.value == operation.drain_marker
        ));
    }
}
