//! Installed `audio/play` realization that retains one bounded WAV artifact.

use super::audio_play_operation::DRAIN_MARKER;
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedValueStore, OperationAction, OperationInput, PortId, RequestId,
    ValueRef, ValueStorage,
};

pub(super) const HOST_OPERATION: &str = conduit_std_offers::AUDIO_WAV_ARTIFACT_OPERATION;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::AUDIO_WAV_ARTIFACT_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct WavArtifactOperation {
    pending: Option<RequestId>,
    next_request: u32,
    drain_marker: ValueRef,
    draining: bool,
    closed: bool,
}

impl WavArtifactOperation {
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
                    < u32::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS) =>
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
                    return InstalledOperation::fail(182);
                }
                if self.draining {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => InstalledOperation::fail(181),
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
        let Ok(input) = BoundedValueRef::new(
            value,
            conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
        ) else {
            return InstalledOperation::fail(183);
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
        host_requests: usize::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS) + 1,
        sign_items: 64,
        maximum_value_bytes: conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let drain_marker = values
        .store(&DRAIN_MARKER)
        .map_err(|error| format!("store WAV artifact drain marker: {error:?}"))?;
    Ok(InstalledOperation::WavArtifact(WavArtifactOperation {
        pending: None,
        next_request: 0,
        drain_marker,
        draining: false,
        closed: false,
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::audio_write_wav_artifact_offer();
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
            binding.class_id.as_str() != conduit_std_offers::AUDIO_WAV_ARTIFACT_RESOURCE_CLASS
                || binding.units != 1
                || binding.protected.is_some()
                || binding.compute.is_some()
        })
        || placement.authority.len() != 1
        || authority.is_none_or(|binding| {
            binding.contract_id.as_str()
                != conduit_std_offers::AUDIO_WAV_ARTIFACT_AUTHORITY_CONTRACT
                || binding.host_operation_contract_id.as_str() != HOST_OPERATION
                || binding.subject_kind.as_str() != conduit_audio::AUDIO_PCM_INFO_ID
                || binding.host_id != placement.host_id
                || binding.boot_id != placement.boot_id
                || binding.capability_id != placement.capability_id
        })
        || !placement.configuration.is_empty()
    {
        return Err(
            "planned WAV artifact identity/resource/authority differs from installation".into(),
        );
    }
    Ok(())
}

pub(super) fn prepare_session(
    placement: &PlannedGear,
    selected: Option<&crate::hosted_wav_artifact::WavArtifactSelection>,
) -> Result<crate::hosted_wav_artifact::WavArtifactSession, String> {
    validate(placement)?;
    let selected =
        selected.ok_or_else(|| "planned WAV artifact has no exact destination".to_string())?;
    if selected.boot_id != placement.boot_id
        || selected.offer_generation != placement.offer_generation
        || placement.resources[0].pool_id != selected.pool_id()
    {
        return Err("planned WAV artifact destination is stale or differs from selection".into());
    }
    Ok(crate::hosted_wav_artifact::WavArtifactSession::prepare(
        selected.clone(),
    ))
}

pub(super) fn execute(
    session: &mut crate::hosted_wav_artifact::WavArtifactSession,
    input: &[u8],
) -> HostOperationOutcome {
    let result = if input == DRAIN_MARKER {
        session.finish()
    } else {
        session.write_frame(input)
    };
    match result {
        Ok(()) => HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: None,
            failure: None,
        },
        Err(_) => HostOperationOutcome {
            disposition: HostOperationDisposition::Failed,
            output: None,
            failure: Some(Failure {
                code: FailureCode::HostOperationFailed,
                detail: 184,
            }),
        },
    }
}
