//! Exact finite named-pattern storage offer.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ResourceRequirement,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const TEMPLATE_STORAGE_STD_PROFILE: &str = "std/named-pattern-storage-kernel-hosted@1";
pub const TEMPLATE_STORAGE_STD_IMPLEMENTATION: &str = "std/kernel-named-pattern-storage@1";
pub const TEMPLATE_STORAGE_STD_ARTIFACT: &str = "conduit-std-host/named-pattern-storage@1";
pub const TEMPLATE_STORAGE_HOST_OPERATION: &str = "conduit.host/named-pattern-storage@1";
pub const TEMPLATE_STORAGE_RESOURCE_CLASS: &str = "conduit.resource/named-pattern-storage-slot@1";

pub fn template_storage_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::named_pattern_template_storage_semantic_contract();
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("named-pattern-storage"),
            execution_profile_id: ExecutionProfileId::from(TEMPLATE_STORAGE_STD_PROFILE),
            implementation_id: ImplementationId::from(TEMPLATE_STORAGE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TEMPLATE_STORAGE_STD_ARTIFACT),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(TEMPLATE_STORAGE_HOST_OPERATION),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: vec![ResourceRequirement {
                content: None,
                class_id: conduit_core::ResourceClassId::from(TEMPLATE_STORAGE_RESOURCE_CLASS),
                units: 1,
                protected_role: None,
                compute: None,
            }],
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_portable_front_and_requires_one_finite_storage_slot() {
        let definition = conduit_semantic_catalog::named_pattern_template_storage_definition();
        let offer = template_storage_std_offer();
        assert_eq!(offer.inputs, definition.inputs);
        assert_eq!(offer.outputs, definition.outputs);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.resource_requirements[0].units, 1);
        assert!(offer.authority_requirements.is_empty());
    }
}
