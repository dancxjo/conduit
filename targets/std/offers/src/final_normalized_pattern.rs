//! Allocation-free std-kernel realization of final normalized-pattern selection.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, ImplementationId,
};

pub const FINAL_NORMALIZED_PATTERN_STD_PROFILE: &str = "std/final-normalized-pattern-kernel@1";
pub const FINAL_NORMALIZED_PATTERN_STD_IMPLEMENTATION: &str =
    "std/kernel-final-normalized-pattern@1";
pub const FINAL_NORMALIZED_PATTERN_STD_ARTIFACT: &str =
    "conduit-std-host/final-normalized-pattern@1";

pub fn final_normalized_pattern_std_offer() -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        conduit_semantic_catalog::final_normalized_pattern_semantic_contract(),
        CapabilityRealization {
            capability_id: CapabilityId::from("final-normalized-pattern"),
            execution_profile_id: ExecutionProfileId::from(FINAL_NORMALIZED_PATTERN_STD_PROFILE),
            implementation_id: ImplementationId::from(FINAL_NORMALIZED_PATTERN_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FINAL_NORMALIZED_PATTERN_STD_ARTIFACT),
            host_operations: Vec::new(),
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
    fn offer_preserves_exact_temporal_and_value_front() {
        let definition = conduit_semantic_catalog::final_normalized_pattern_definition();
        let offer = final_normalized_pattern_std_offer();
        assert_eq!(offer.inputs, definition.inputs);
        assert_eq!(offer.outputs, definition.outputs);
        assert!(offer.host_operations.is_empty());
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}
