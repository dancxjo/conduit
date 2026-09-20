//! Exact finite sequence-normalization offer owned by the hosted std Host.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const NORMALIZE_SEQUENCE_STD_PROFILE: &str = "std/normalize-relative-duration-kernel-hosted@1";
pub const NORMALIZE_SEQUENCE_STD_IMPLEMENTATION: &str = "std/kernel-normalize-relative-duration@1";
pub const NORMALIZE_SEQUENCE_STD_ARTIFACT: &str = "conduit-std-host/normalize-relative-duration@1";
pub const NORMALIZE_SEQUENCE_HOST_OPERATION: &str = "conduit.host/normalize-relative-duration@1";

pub fn normalize_sequence_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::normalize_relative_duration_semantic_contract();
    let target_kind = contract.kind_id.clone();
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from("normalize-relative-duration"),
            execution_profile_id: ExecutionProfileId::from(NORMALIZE_SEQUENCE_STD_PROFILE),
            implementation_id: ImplementationId::from(NORMALIZE_SEQUENCE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(NORMALIZE_SEQUENCE_STD_ARTIFACT),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(NORMALIZE_SEQUENCE_HOST_OPERATION),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
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
    fn offer_preserves_the_portable_normalization_front() {
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
