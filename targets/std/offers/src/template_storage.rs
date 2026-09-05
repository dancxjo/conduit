//! Exact finite named-pattern storage offer.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    ResourceRequirement, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const TEMPLATE_STORAGE_STD_PROFILE: &str = "std/named-pattern-storage-kernel-hosted@1";
pub const TEMPLATE_STORAGE_STD_IMPLEMENTATION: &str = "std/kernel-named-pattern-storage@1";
pub const TEMPLATE_STORAGE_STD_ARTIFACT: &str = "conduit-std-host/named-pattern-storage@1";
pub const TEMPLATE_STORAGE_HOST_OPERATION: &str = "conduit.host/named-pattern-storage@1";
pub const TEMPLATE_STORAGE_RESOURCE_CLASS: &str = "conduit.resource/named-pattern-storage-slot@1";

pub fn template_storage_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::named_pattern_template_storage_definition();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "maximum-commands".into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("named-pattern-storage"),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TEMPLATE_STORAGE_STD_PROFILE),
            implementation_id: ImplementationId::from(TEMPLATE_STORAGE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TEMPLATE_STORAGE_STD_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(TEMPLATE_STORAGE_HOST_OPERATION),
            target_kind: Some(contract.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: vec![ResourceRequirement {
            class_id: conduit_core::ResourceClassId::from(TEMPLATE_STORAGE_RESOURCE_CLASS),
            units: 1,
            protected_role: None,
            compute: None,
        }],
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_semantic_catalog::MAXIMUM_TEMPLATE_STORAGE_COMMANDS as u16,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
                * (conduit_semantic_catalog::MAXIMUM_TEMPLATE_STORAGE_COMMANDS as u32 + 1),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_portable_face_and_requires_one_finite_storage_slot() {
        let definition = conduit_semantic_catalog::named_pattern_template_storage_definition();
        let offer = template_storage_std_offer();
        assert_eq!(offer.inputs, definition.inputs);
        assert_eq!(offer.outputs, definition.outputs);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.resource_requirements[0].units, 1);
        assert!(offer.authority_requirements.is_empty());
    }
}
