//! Hosted std realization of the provider-neutral House prompt projection.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer,
};

pub const HOUSE_PROMPT_STD_IMPLEMENTATION: &str = "std/kernel-house-context-to-prompt@1";
pub const HOUSE_PROMPT_STD_PROFILE: &str = "std/house-context-to-prompt-kernel@1";
pub const HOUSE_PROMPT_STD_ARTIFACT: &str = "conduit-std-host/house-context-to-prompt@1";
pub const HOUSE_PROMPT_DETECTION_OPERATION: &str = "conduit.host/house-prompt-detection@1";
pub const HOUSE_PROMPT_CONTEXT_OPERATION: &str = "conduit.host/house-prompt-context@1";

pub fn house_prompt_std_offer() -> CapabilityOffer {
    let contract = conduit_tongues::house_prompt_contract();
    let operation = |contract_id, maximum_input_bytes| HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract_id),
        target_kind: Some(contract.kind_id.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes,
        maximum_output_bytes: conduit_tongues::MAXIMUM_HOUSE_PROMPT_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("house-context-to-prompt"),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(HOUSE_PROMPT_STD_PROFILE),
            implementation_id: ImplementationId::from(HOUSE_PROMPT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(HOUSE_PROMPT_STD_ARTIFACT),
        },
        host_operations: vec![
            operation(
                HOUSE_PROMPT_CONTEXT_OPERATION,
                conduit_tongues::MAXIMUM_WIRED_HOUSE_CONTEXT_VALUE_BYTES as u32,
            ),
            operation(
                HOUSE_PROMPT_DETECTION_OPERATION,
                conduit_tongues::MAXIMUM_ADDRESS_DETECTION_VALUE_BYTES as u32,
            ),
        ],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_portable_contract_and_finite_host_calls() {
        let portable = conduit_tongues::house_prompt_contract();
        let offer = house_prompt_std_offer();
        assert_eq!(offer.kind_id, portable.kind_id);
        assert_eq!(offer.inputs, portable.inputs);
        assert_eq!(offer.outputs, portable.outputs);
        assert_eq!(offer.host_operations.len(), 2);
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}
