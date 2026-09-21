//! Hosted std realization of validated model-result text projection.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId,
};

pub const MODEL_RESULT_TO_TEXT_STD_IMPLEMENTATION: &str = "std/model-result-to-text@1";
pub const MODEL_RESULT_TO_TEXT_STD_PROFILE: &str = "std/model-result-to-text-hosted@1";
pub const MODEL_RESULT_TO_TEXT_STD_ARTIFACT: &str = "conduit-std-host/model-result-to-text@1";
pub const MODEL_RESULT_TO_TEXT_OPERATION: &str = "conduit.host/model-result-to-text@1";
pub const GENERATED_CHUNK_TO_TEXT_OPERATION: &str = "conduit.host/generated-chunk-to-text@1";

pub fn model_result_to_text_std_offer() -> CapabilityOffer {
    model_text_offer(
        conduit_ai::model_result_to_text_contract(),
        "model-result-to-text",
        MODEL_RESULT_TO_TEXT_OPERATION,
        conduit_ai::MAXIMUM_MODEL_RESULT_ENVELOPE_BYTES,
        conduit_ai::MAXIMUM_MODEL_TEXT_BYTES,
    )
}

pub fn model_result_flow_to_text_std_offer() -> CapabilityOffer {
    model_text_offer(
        conduit_ai::model_result_flow_to_text_contract(),
        "model-result-flow-to-text",
        MODEL_RESULT_TO_TEXT_OPERATION,
        conduit_ai::MAXIMUM_MODEL_RESULT_ENVELOPE_BYTES,
        conduit_ai::MAXIMUM_MODEL_TEXT_BYTES,
    )
}

pub fn generated_chunk_to_text_std_offer() -> CapabilityOffer {
    model_text_offer(
        conduit_ai::generated_chunk_to_text_contract(),
        "generated-chunk-to-text",
        GENERATED_CHUNK_TO_TEXT_OPERATION,
        conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES as u32,
        conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNK_BYTES as u32,
    )
}

fn model_text_offer(
    contract: conduit_ai::ModelTextContract,
    capability: &str,
    operation: &str,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
) -> CapabilityOffer {
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract.into_semantic_capability_contract(),
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(MODEL_RESULT_TO_TEXT_STD_PROFILE),
            implementation_id: ImplementationId::from(MODEL_RESULT_TO_TEXT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(MODEL_RESULT_TO_TEXT_STD_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(operation),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes,
                maximum_output_bytes,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_front_and_finite_projection_boundary() {
        for (offer, contract) in [
            (
                model_result_to_text_std_offer(),
                conduit_ai::model_result_to_text_contract(),
            ),
            (
                model_result_flow_to_text_std_offer(),
                conduit_ai::model_result_flow_to_text_contract(),
            ),
            (
                generated_chunk_to_text_std_offer(),
                conduit_ai::generated_chunk_to_text_contract(),
            ),
        ] {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
            assert_eq!(offer.host_calls.len(), 1);
            assert!(offer.resource_requirements.is_empty());
            assert!(offer.authority_requirements.is_empty());
        }
    }
}
