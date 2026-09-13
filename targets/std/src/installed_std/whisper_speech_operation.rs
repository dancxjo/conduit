//! Installed operation for initialized hosted Whisper speech recognition.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, ResourceClassId};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::WHISPER_SPEECH_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static CLIP_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION,
    budget: clip_budget,
    prepare: prepare_clip,
};

pub(super) struct WhisperSpeechOperation {
    pending: bool,
    emitted: bool,
    maximum_input_bytes: u32,
}

impl WhisperSpeechOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending && request == RequestId(0) =>
            {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 2)
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 3),
                    _ => fail(FailureCode::HostOperationFailed, 4),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 5),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

pub(super) fn execute(
    adapter: Option<&mut crate::hosted_speech_recognition::WhisperSpeechAdapter>,
    input: &[u8],
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, crate::hosted_speech_recognition::WhisperFailure> {
    adapter
        .ok_or(crate::hosted_speech_recognition::WhisperFailure::MissingProvider)?
        .recognize(input, cancelled)
}

pub(super) fn execute_clip(
    adapter: Option<&mut crate::hosted_speech_recognition::WhisperSpeechAdapter>,
    input: &[u8],
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, crate::hosted_speech_recognition::WhisperFailure> {
    adapter
        .ok_or(crate::hosted_speech_recognition::WhisperFailure::MissingProvider)?
        .recognize_clip(input, cancelled)
}

pub(super) fn failure_outcome(
    failure: crate::hosted_speech_recognition::WhisperFailure,
) -> HostOperationOutcome {
    use crate::hosted_speech_recognition::WhisperFailure as Whisper;
    let (disposition, code, detail) = match failure {
        Whisper::MissingProvider => (
            HostOperationDisposition::Denied,
            FailureCode::HostOperationDenied,
            1,
        ),
        Whisper::InvalidPcm
        | Whisper::InvalidClip
        | Whisper::UnsupportedPcmProfile
        | Whisper::AudioOverflow => (
            HostOperationDisposition::Failed,
            FailureCode::InvalidInput,
            2,
        ),
        Whisper::Cancelled => (
            HostOperationDisposition::Cancelled,
            FailureCode::Cancelled,
            3,
        ),
        Whisper::Timeout => (
            HostOperationDisposition::Failed,
            FailureCode::HostOperationFailed,
            4,
        ),
        Whisper::OutputOverflow => (
            HostOperationDisposition::Failed,
            FailureCode::WorkBudgetExhausted,
            5,
        ),
        _ => (
            HostOperationDisposition::Failed,
            FailureCode::HostOperationFailed,
            6,
        ),
    };
    HostOperationOutcome {
        disposition,
        output: None,
        failure: Some(Failure { code, detail }),
    }
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::whisper_speech_offer();
    validate_offer(placement, &offer)
}

fn validate_offer(
    placement: &PlannedGear,
    offer: &conduit_core::CapabilityOffer,
) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
        || placement.resources.len() != 1
        || placement.resources[0].class_id
            != ResourceClassId::from(conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS)
        || placement.resources[0].units != 1
    {
        return Err("planned Whisper speech recognition does not match its installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
    })
}

fn clip_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_offer(placement, &conduit_std_offers::whisper_clip_speech_offer())?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::WhisperSpeech(WhisperSpeechOperation {
        pending: false,
        emitted: false,
        maximum_input_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
    }))
}

fn prepare_clip(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_offer(placement, &conduit_std_offers::whisper_clip_speech_offer())?;
    Ok(InstalledOperation::WhisperSpeech(WhisperSpeechOperation {
        pending: false,
        emitted: false,
        maximum_input_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
