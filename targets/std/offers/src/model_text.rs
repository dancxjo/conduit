//! Hosted std realization of validated model-result text projection.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer,
};

pub const MODEL_RESULT_TO_TEXT_STD_IMPLEMENTATION: &str = "std/model-result-to-text@1";
pub const MODEL_RESULT_TO_TEXT_STD_PROFILE: &str = "std/model-result-to-text-hosted@1";
pub const MODEL_RESULT_TO_TEXT_STD_ARTIFACT: &str = "conduit-std-host/model-result-to-text@1";
pub const MODEL_RESULT_TO_TEXT_OPERATION: &str = "conduit.host/model-result-to-text@1";

pub fn model_result_to_text_std_offer() -> CapabilityOffer {
    let contract = conduit_ai::model_result_to_text_contract();
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("model-result-to-text"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(MODEL_RESULT_TO_TEXT_STD_PROFILE),
            implementation_id: ImplementationId::from(MODEL_RESULT_TO_TEXT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(MODEL_RESULT_TO_TEXT_STD_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(MODEL_RESULT_TO_TEXT_OPERATION),
            target_kind: Some(conduit_core::kind_id(conduit_ai::MODEL_RESULT_TO_TEXT_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_ai::MAXIMUM_MODEL_RESULT_ENVELOPE_BYTES,
            maximum_output_bytes: conduit_ai::MAXIMUM_MODEL_TEXT_BYTES,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_face_and_finite_projection_boundary() {
        let offer = model_result_to_text_std_offer();
        let contract = conduit_ai::model_result_to_text_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.host_operations.len(), 1);
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}
