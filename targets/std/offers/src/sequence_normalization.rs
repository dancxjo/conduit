//! Exact finite sequence-normalization offer owned by the hosted std Host.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const NORMALIZE_SEQUENCE_STD_PROFILE: &str = "std/normalize-relative-duration-kernel-hosted@1";
pub const NORMALIZE_SEQUENCE_STD_IMPLEMENTATION: &str = "std/kernel-normalize-relative-duration@1";
pub const NORMALIZE_SEQUENCE_STD_ARTIFACT: &str = "conduit-std-host/normalize-relative-duration@1";
pub const NORMALIZE_SEQUENCE_HOST_OPERATION: &str = "conduit.host/normalize-relative-duration@1";

pub fn normalize_sequence_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::normalize_relative_duration_definition();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("normalize-relative-duration"),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(NORMALIZE_SEQUENCE_STD_PROFILE),
            implementation_id: ImplementationId::from(NORMALIZE_SEQUENCE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(NORMALIZE_SEQUENCE_STD_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(NORMALIZE_SEQUENCE_HOST_OPERATION),
            target_kind: Some(contract.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_normalization_face() {
        let contract = conduit_semantic_catalog::normalize_relative_duration_definition();
        let offer = normalize_sequence_std_offer();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}
