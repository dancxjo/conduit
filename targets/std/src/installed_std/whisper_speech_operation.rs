//! Installed operation for initialized hosted Whisper speech recognition.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, ResourceClassId};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    PortId, RequestId,
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

impl<const PORTS: usize> StepOperation<PORTS> for WhisperSpeechOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return step_fail(FailureCode::InvalidLifecycle, 5);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed Whisper completion");
                    io.send(PortId(0), output.value)
                        .expect("ready Whisper output");
                    self.pending = false;
                    self.emitted = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Denied, _, _) => step_fail(FailureCode::HostCallDenied, 2),
                (HostCallDisposition::Cancelled, _, _) => step_fail(FailureCode::Cancelled, 3),
                (_, _, Some(failure)) => StepOutcome::Fail(failure),
                _ => step_fail(FailureCode::HostCallFailed, 4),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return step_fail(FailureCode::InvalidLifecycle, 5);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            io.consume(PortId(0)).expect("present Whisper input");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("Whisper Host Call");
            self.pending = true;
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl WhisperSpeechOperation {}

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
) -> HostCallOutcome {
    use crate::hosted_speech_recognition::WhisperFailure as Whisper;
    let (disposition, code, detail) = match failure {
        Whisper::MissingProvider => (HostCallDisposition::Denied, FailureCode::HostCallDenied, 1),
        Whisper::InvalidPcm
        | Whisper::InvalidClip
        | Whisper::UnsupportedPcmProfile
        | Whisper::AudioOverflow => (HostCallDisposition::Failed, FailureCode::InvalidInput, 2),
        Whisper::Cancelled => (HostCallDisposition::Cancelled, FailureCode::Cancelled, 3),
        Whisper::Timeout => (HostCallDisposition::Failed, FailureCode::HostCallFailed, 4),
        Whisper::OutputOverflow => (
            HostCallDisposition::Failed,
            FailureCode::WorkBudgetExhausted,
            5,
        ),
        _ => (HostCallDisposition::Failed, FailureCode::HostCallFailed, 6),
    };
    HostCallOutcome {
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
        || placement.host_calls != offer.host_calls
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
