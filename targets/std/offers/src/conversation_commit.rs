//! Exact std realizations for the two irreversible conversation commit boundaries.

use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder,
    CapabilityRealization, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId,
};

pub const RECOGNIZED_TURN_COMMIT_PROFILE: &str = "std/recognized-turn-commit-kernel@1";
pub const RECOGNIZED_TURN_COMMIT_IMPLEMENTATION: &str = "std/recognized-turn-commit@1";
pub const RECOGNIZED_TURN_COMMIT_ARTIFACT: &str = "conduit-std-host/recognized-turn-commit@1";
pub const RECOGNIZED_TURN_COMMIT_OPERATION: &str = "conduit.host/recognized-turn-commit@1";

pub const GENERATED_SPEECH_COMMIT_PROFILE: &str = "std/generated-speech-commit-kernel@1";
pub const GENERATED_SPEECH_COMMIT_IMPLEMENTATION: &str = "std/generated-speech-commit@1";
pub const GENERATED_SPEECH_COMMIT_ARTIFACT: &str = "conduit-std-host/generated-speech-commit@1";
pub const GENERATED_SPEECH_PUSH_OPERATION: &str = "conduit.host/generated-speech-push@1";
pub const GENERATED_SPEECH_DRAIN_OPERATION: &str = "conduit.host/generated-speech-drain@1";
pub const GENERATED_SPEECH_CLOSE_OPERATION: &str = "conduit.host/generated-speech-close@1";

pub fn recognized_turn_commit_offer() -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        conduit_tongues::committed_recognition_turn_contract().into_semantic_capability_contract(),
        CapabilityRealization {
            capability_id: CapabilityId::from("std-recognized-turn-commit-v1"),
            execution_profile_id: ExecutionProfileId::from(RECOGNIZED_TURN_COMMIT_PROFILE),
            implementation_id: ImplementationId::from(RECOGNIZED_TURN_COMMIT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(RECOGNIZED_TURN_COMMIT_ARTIFACT),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(RECOGNIZED_TURN_COMMIT_OPERATION),
                target_kind: Some(kind_id(conduit_tongues::CHAT_MESSAGE_VALUE_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_tongues::MAXIMUM_RECOGNITION_EVENT_BYTES as u32,
                maximum_output_bytes: conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

pub fn generated_speech_commit_offer() -> CapabilityOffer {
    let operation = |id, input| HostOperationRequirement {
        contract_id: HostOperationContractId::from(id),
        target_kind: Some(kind_id(conduit_tongues::SPEAKABLE_TEXT_VALUE_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: conduit_tongues::SPEECH_COMMIT_QUEUE_BYTES,
    };
    CapabilityOfferBuilder::new(
        conduit_tongues::speech_commit_contract().into_semantic_capability_contract(),
        CapabilityRealization {
            capability_id: CapabilityId::from("std-generated-speech-commit-v1"),
            execution_profile_id: ExecutionProfileId::from(GENERATED_SPEECH_COMMIT_PROFILE),
            implementation_id: ImplementationId::from(GENERATED_SPEECH_COMMIT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(GENERATED_SPEECH_COMMIT_ARTIFACT),
            host_operations: vec![
                operation(
                    GENERATED_SPEECH_PUSH_OPERATION,
                    conduit_tongues::MAXIMUM_TEXT_BYTES,
                ),
                operation(
                    GENERATED_SPEECH_DRAIN_OPERATION,
                    conduit_tongues::MAXIMUM_TEXT_BYTES,
                ),
                operation(
                    GENERATED_SPEECH_CLOSE_OPERATION,
                    conduit_tongues::MAXIMUM_TEXT_BYTES,
                ),
            ],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_offers_preserve_portable_fronts_and_finite_host_calls() {
        let recognized = recognized_turn_commit_offer();
        let recognized_contract = conduit_tongues::committed_recognition_turn_contract();
        assert_eq!(recognized.kind_id, recognized_contract.kind_id);
        assert_eq!(recognized.inputs, recognized_contract.inputs);
        assert_eq!(recognized.outputs, recognized_contract.outputs);
        assert_eq!(recognized.limits, recognized_contract.limits);
        assert_eq!(recognized.host_operations.len(), 1);
        let generated = generated_speech_commit_offer();
        let generated_contract = conduit_tongues::speech_commit_contract();
        assert_eq!(generated.kind_id, generated_contract.kind_id);
        assert_eq!(generated.inputs, generated_contract.inputs);
        assert_eq!(generated.outputs, generated_contract.outputs);
        assert_eq!(generated.limits, generated_contract.limits);
        assert_eq!(generated.host_operations.len(), 3);
        assert!(generated
            .host_operations
            .iter()
            .all(|operation| operation.maximum_in_flight == 1));
    }
}
