//! Proof-only recorded-audio realization of portable speech recognition.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, PlannedGear,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, OperationAction,
    OperationInput, PortId, RequestId,
};

pub(crate) const IMPLEMENTATION: &str = "conduit-proof/recorded-speech-recognizer@1";
const PROFILE: &str = "conduit-proof/recorded-speech-recognizer-hosted@1";
const ARTIFACT: &str = "conduit-std-host/proof-recorded-speech-recognizer@1";
pub(crate) const HOST_CALL: &str = "conduit.host/proof-recorded-speech-recognize@1";

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecordedSpeechOperation {
    pending: bool,
    emitted: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for RecordedSpeechOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return step_fail(FailureCode::InvalidLifecycle, 4);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed recorded-speech completion");
                    io.send(PortId(0), output.value)
                        .expect("ready recorded recognition output");
                    self.pending = false;
                    self.emitted = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Denied, _, _) => step_fail(FailureCode::HostCallDenied, 2),
                (_, _, Some(failure)) => StepOutcome::Fail(failure),
                _ => step_fail(FailureCode::HostCallFailed, 3),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return step_fail(FailureCode::InvalidLifecycle, 4);
            }
            let Ok(input) = BoundedValueRef::new(
                value,
                conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
            ) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            io.consume(PortId(0))
                .expect("present recorded speech input");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("recorded speech Host Call");
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

impl RecordedSpeechOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(
                    value,
                    conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
                ) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                self.pending = true;
                OperationAction::RequestHostCall {
                    request: RequestId(0),
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending && request == RequestId(0) =>
            {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostCallDisposition::Denied, _, _) => fail(FailureCode::HostCallDenied, 2),
                    _ => fail(FailureCode::HostCallFailed, 3),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 4),
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

pub(crate) fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_tongues::speech_recognition_contract().into_semantic_capability_contract(),
        Back {
            capability_id: CapabilityId::from("proof-recorded-speech-recognizer"),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(HOST_CALL),
                target_kind: Some(kind_id(conduit_tongues::SPEECH_RECOGNIZE_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
                maximum_output_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

pub(super) struct RecordedSpeechHost {
    recognizer: conduit_tongues::RecordedSpeechRecognizer,
    output: Vec<u8>,
}

impl RecordedSpeechHost {
    pub(super) fn execute(&mut self, input: &[u8]) -> Result<&[u8], String> {
        match self
            .recognizer
            .recognize(input)
            .map_err(|error| format!("recorded recognition input: {error:?}"))?
        {
            conduit_tongues::SpeechRecognitionAttempt::Result(result) => {
                self.output = conduit_tongues::encode_speech_recognition_result(&result)
                    .map_err(|error| format!("recorded recognition result: {error:?}"))?;
                Ok(&self.output)
            }
            conduit_tongues::SpeechRecognitionAttempt::ResourceUnavailable => {
                Err("recorded recognizer resource is unavailable".into())
            }
            conduit_tongues::SpeechRecognitionAttempt::Failed { .. } => {
                Err("recorded recognizer has no matching fixture".into())
            }
        }
    }
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Result<Vec<Option<RecordedSpeechHost>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str() != IMPLEMENTATION {
                return Ok(None);
            }
            let audio = super::test_local_model_io::recorded_house_audio()?;
            let recognizer = conduit_tongues::RecordedSpeechRecognizer::new(&[(
                &audio,
                "Rosehip House, what is the temperature upstairs?",
            )])
            .map_err(|error| format!("prepare recorded recognizer: {error:?}"))?;
            Ok(Some(RecordedSpeechHost {
                recognizer,
                output: Vec::with_capacity(conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES),
            }))
        })
        .collect()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id.as_str() != PROFILE
        || placement.implementation_id.as_str() != IMPLEMENTATION
        || placement.artifact_id.as_str() != ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || !placement.configuration.is_empty()
    {
        return Err("planned recorded recognizer does not match proof installation".into());
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

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::RecordedSpeech(
        RecordedSpeechOperation {
            pending: false,
            emitted: false,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_fixture_realization_preserves_owner_issued_front() {
        let contract =
            conduit_tongues::speech_recognition_contract().into_semantic_capability_contract();
        let offer = offer();
        assert_eq!(offer.startup_parameters, contract.startup_parameters);
        assert_eq!(offer.shorthand, contract.shorthand);
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_calls.len(), 1);
    }
}
