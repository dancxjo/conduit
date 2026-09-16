//! Browser-owned bounded PCM capture and safe playback installations.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PlannedGear,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(crate) const CAPTURE_IMPLEMENTATION: &str = "browser/pcm-push-to-talk@1";
pub(crate) const PLAY_IMPLEMENTATION: &str = "browser/webaudio-safe-pcm@1";
const PROFILE: &str = "browser/pcm-human-audio@1";
const ARTIFACT: &str = "conduit-browser-runtime/pcm-human-audio@1";
pub(crate) const CAPTURE_OPERATION: &str = "conduit.host/browser-pcm-push-to-talk@1";
pub(crate) const PLAY_OPERATION: &str = "conduit.host/browser-safe-pcm-playback@1";
pub(crate) const CAPTURE_RESOURCE: &str = "conduit.resource/browser-microphone-turn@1";
pub(crate) const PLAY_RESOURCE: &str = "conduit.resource/browser-audio-output@1";
pub(crate) const CAPTURE_POOL: &str = "browser/microphone-turn";
pub(crate) const PLAY_POOL: &str = "browser/audio-output";
pub(crate) const CAPTURE_AUTHORITY: &str = "conduit.authority/request-browser-microphone@1";
pub(crate) const PLAY_AUTHORITY: &str = "conduit.authority/use-browser-audio-output@1";
pub(crate) const MAXIMUM_SAFE_GAIN_MILLIONTHS: u32 = 50_000;
pub(crate) const MAXIMUM_CAPTURE_REQUESTS: u32 = 8_192;

pub(crate) static CAPTURE: BrowserInstallation = BrowserInstallation {
    implementation_id: CAPTURE_IMPLEMENTATION,
    offer: capture_offer,
    prepare: prepare_capture,
    perform: None,
};

pub(crate) static PLAYBACK: BrowserInstallation = BrowserInstallation {
    implementation_id: PLAY_IMPLEMENTATION,
    offer: playback_offer,
    prepare: prepare_playback,
    perform: None,
};

fn identity(implementation: &str) -> ImplementationOffer {
    ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from(PROFILE),
        implementation_id: ImplementationId::from(implementation),
        artifact_id: ArtifactId::from(ARTIFACT),
    }
}

pub(crate) fn capture_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::audio_capture_push_to_talk_contract();
    CapabilityOffer {
        startup_parameters: conduit_semantic_catalog::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(CAPTURE_IMPLEMENTATION),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: identity(CAPTURE_IMPLEMENTATION),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(CAPTURE_OPERATION),
            target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
        }],
        resource_requirements: vec![resource_requirement(CAPTURE_RESOURCE, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(CAPTURE_AUTHORITY),
            host_operation_contract_id: HostOperationContractId::from(CAPTURE_OPERATION),
            subject_kind: kind_id(conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_KIND),
        }],
        limits: contract.limits,
    }
}

pub(crate) fn playback_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::audio_play_contract();
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(PLAY_IMPLEMENTATION),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::AUDIO_PLAY_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: identity(PLAY_IMPLEMENTATION),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(PLAY_OPERATION),
            target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![resource_requirement(PLAY_RESOURCE, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(PLAY_AUTHORITY),
            host_operation_contract_id: HostOperationContractId::from(PLAY_OPERATION),
            subject_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        }],
        limits: contract.limits,
    }
}

fn prepare_capture(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &capture_offer())?;
    let request = values
        .store(&[0])
        .map_err(|error| format!("store browser microphone request: {error:?}"))?;
    Ok(BrowserOperation::installed(CaptureOperation {
        request,
        next: 0,
        pending: false,
        completed: false,
    }))
}

fn prepare_playback(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &playback_offer())?;
    Ok(BrowserOperation::installed(PlaybackOperation {
        next: 0,
        pending: None,
        input_closed: false,
    }))
}

struct CaptureOperation {
    request: ValueRef,
    next: u32,
    pending: bool,
    completed: bool,
}

impl CaptureOperation {
    fn request(&mut self) -> OperationAction {
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(self.next),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.request, 1).expect("capture request is one byte"),
        }
    }
}

impl Operation for CaptureOperation {
    fn start(&mut self) -> OperationAction {
        self.request()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending && request == RequestId(self.next) =>
            {
                self.pending = false;
                if outcome.disposition != HostOperationDisposition::Completed
                    || outcome.failure.is_some()
                {
                    return host_failure(outcome.disposition);
                }
                match outcome.output {
                    Some(output) => OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    },
                    None => {
                        self.completed = true;
                        OperationAction::Complete
                    }
                }
            }
            _ => invalid(1),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.completed || self.pending {
            return invalid(2);
        }
        let Some(next) = self.next.checked_add(1) else {
            return identity_exhausted();
        };
        if next >= MAXIMUM_CAPTURE_REQUESTS {
            self.completed = true;
            return OperationAction::Complete;
        }
        self.next = next;
        self.request()
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.completed = true;
    }
}

struct PlaybackOperation {
    next: u32,
    pending: Option<RequestId>,
    input_closed: bool,
}

impl Operation for PlaybackOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                self.input_closed = true;
                OperationAction::Complete
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                if outcome.disposition != HostOperationDisposition::Completed
                    || outcome.failure.is_some()
                    || outcome.output.is_some()
                {
                    host_failure(outcome.disposition)
                } else if self.input_closed {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => invalid(3),
        }
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        if port != PortId(0) || self.pending.is_some() || self.input_closed {
            return invalid(4);
        }
        let Ok((header, payload)) = conduit_audio::PcmFrameHeader::decode_frame(canonical) else {
            return invalid(5);
        };
        if header.validate_payload(payload).is_err() {
            return invalid(6);
        }
        let request = RequestId(self.next);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(
                value,
                conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                    + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
            )
            .expect("validated PCM fits the playback operation"),
        }
    }

    fn advance(&mut self) -> OperationAction {
        let Some(next) = self.next.checked_add(1) else {
            return identity_exhausted();
        };
        self.next = next;
        if self.input_closed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.input_closed = true;
    }
}

fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

fn host_failure(disposition: HostOperationDisposition) -> OperationAction {
    let code = match disposition {
        HostOperationDisposition::Denied => FailureCode::HostOperationDenied,
        HostOperationDisposition::Cancelled => FailureCode::Cancelled,
        _ => FailureCode::HostOperationFailed,
    };
    OperationAction::Fail(Failure { code, detail: 7 })
}

fn identity_exhausted() -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::IdentityCapacityExhausted,
        detail: 8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_pcm_offers_seal_distinct_resource_and_authority_truth() {
        let capture = capture_offer();
        assert_eq!(
            capture.kind_id.as_str(),
            conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_KIND
        );
        assert_eq!(
            capture.host_operations[0].contract_id.as_str(),
            CAPTURE_OPERATION
        );
        assert_eq!(
            capture.resource_requirements[0].class_id.as_str(),
            CAPTURE_RESOURCE
        );
        assert_eq!(
            capture.authority_requirements[0].contract_id.as_str(),
            CAPTURE_AUTHORITY
        );
        assert_eq!(
            capture.outputs[0].temporal,
            conduit_core::PortTemporal::Flow { closes: true }
        );
        assert!(MAXIMUM_CAPTURE_REQUESTS > 0);

        let playback = playback_offer();
        assert_eq!(
            playback.kind_id.as_str(),
            conduit_semantic_catalog::AUDIO_PLAY_KIND
        );
        assert_eq!(
            playback.host_operations[0].contract_id.as_str(),
            PLAY_OPERATION
        );
        assert_eq!(
            playback.resource_requirements[0].class_id.as_str(),
            PLAY_RESOURCE
        );
        assert_eq!(
            playback.authority_requirements[0].contract_id.as_str(),
            PLAY_AUTHORITY
        );
        assert!(MAXIMUM_SAFE_GAIN_MILLIONTHS <= 50_000);
    }
}
