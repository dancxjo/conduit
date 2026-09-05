//! Allocation-free std-kernel realization of final normalized-pattern selection.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    ImplementationId, ImplementationOffer,
};

pub const FINAL_NORMALIZED_PATTERN_STD_PROFILE: &str = "std/final-normalized-pattern-kernel@1";
pub const FINAL_NORMALIZED_PATTERN_STD_IMPLEMENTATION: &str =
    "std/kernel-final-normalized-pattern@1";
pub const FINAL_NORMALIZED_PATTERN_STD_ARTIFACT: &str =
    "conduit-std-host/final-normalized-pattern@1";

pub fn final_normalized_pattern_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::final_normalized_pattern_definition();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "maximum-values".into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("final-normalized-pattern"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(FINAL_NORMALIZED_PATTERN_STD_PROFILE),
            implementation_id: ImplementationId::from(FINAL_NORMALIZED_PATTERN_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FINAL_NORMALIZED_PATTERN_STD_ARTIFACT),
        },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: conduit_semantic_catalog::final_normalized_pattern_limits(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_exact_temporal_and_value_face() {
        let definition = conduit_semantic_catalog::final_normalized_pattern_definition();
        let offer = final_normalized_pattern_std_offer();
        assert_eq!(offer.inputs, definition.inputs);
        assert_eq!(offer.outputs, definition.outputs);
        assert!(offer.host_operations.is_empty());
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}
