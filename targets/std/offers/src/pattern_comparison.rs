//! Exact finite normalized-pattern comparison offer.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const COMPARE_PATTERN_STD_PROFILE: &str = "std/compare-normalized-pattern-kernel-hosted@1";
pub const COMPARE_PATTERN_STD_IMPLEMENTATION: &str = "std/kernel-compare-normalized-pattern@1";
pub const COMPARE_PATTERN_STD_ARTIFACT: &str = "conduit-std-host/compare-normalized-pattern@1";
pub const COMPARE_PATTERN_CANDIDATE_OPERATION: &str = "conduit.host/compare-pattern-candidate@1";
pub const COMPARE_PATTERN_TEMPLATE_OPERATION: &str = "conduit.host/compare-pattern-template@1";

pub fn compare_pattern_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::compare_normalized_pattern_definition();
    CapabilityOffer {
        startup_parameters: vec![
            FaceStartupParameter {
                name: "metric".into(),
                value_type: "Text".into(),
                has_default: true,
            },
            FaceStartupParameter {
                name: "tolerance-millionths".into(),
                value_type: "Count".into(),
                has_default: true,
            },
        ],
        shorthand: None,
        capability_id: CapabilityId::from("compare-normalized-pattern"),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(COMPARE_PATTERN_STD_PROFILE),
            implementation_id: ImplementationId::from(COMPARE_PATTERN_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(COMPARE_PATTERN_STD_ARTIFACT),
        },
        host_operations: vec![
            host_operation(COMPARE_PATTERN_CANDIDATE_OPERATION, &contract.kind_id),
            host_operation(COMPARE_PATTERN_TEMPLATE_OPERATION, &contract.kind_id),
        ],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 2,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 3) as u32,
        },
    }
}

fn host_operation(contract: &str, kind: &conduit_core::KindId) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: Some(kind.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_face_configuration_and_finite_operations() {
        let definition = conduit_semantic_catalog::compare_normalized_pattern_definition();
        let offer = compare_pattern_std_offer();
        assert_eq!(offer.kind_id, definition.kind_id);
        assert_eq!(offer.inputs, definition.inputs);
        assert_eq!(offer.outputs, definition.outputs);
        assert_eq!(offer.host_operations.len(), 2);
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}
