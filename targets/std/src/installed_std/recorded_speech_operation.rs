//! Proof-only recorded-audio realization of portable speech recognition.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    PlannedGear,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(crate) const IMPLEMENTATION: &str = "conduit-proof/recorded-speech-recognizer@1";
const PROFILE: &str = "conduit-proof/recorded-speech-recognizer-hosted@1";
const ARTIFACT: &str = "conduit-std-host/proof-recorded-speech-recognizer@1";
pub(crate) const HOST_OPERATION: &str = "conduit.host/proof-recorded-speech-recognize@1";

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecordedSpeechOperation {
    pending: bool,
    emitted: bool,
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
                    _ => fail(FailureCode::HostOperationFailed, 3),
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
    let contract = conduit_tongues::speech_recognition_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("proof-recorded-speech-recognizer"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(kind_id(conduit_tongues::SPEECH_RECOGNIZE_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
            maximum_output_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
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
        || placement.host_operations != offer.host_operations
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
