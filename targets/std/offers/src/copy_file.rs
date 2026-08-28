//! Exact protected-file copy realization offers owned by the hosted std Host.

use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, protected_resource_requirement,
    resource_requirement, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    PRESENTATION_RESOURCE_CLASS,
};

pub const COPY_FILE_EXECUTION_PROFILE: &str = "conduit.std/file-copy-kernel-hosted@1";
pub const COPY_FILE_IMPLEMENTATION: &str = "std/kernel-file-copy@1";
pub const COPY_FILE_ARTIFACT: &str = "conduit-std-host/file-copy@1";
pub const COPY_FILE_CAPABILITY: &str = "file-copy-v1";
pub const COPY_FILE_HOST_OPERATION_CONTRACT: &str = "conduit.host/file-copy-step@1";
pub const COPY_COMMAND_BYTES: u32 = 1;
pub const COPY_RESULT_PRESENTATION_IMPLEMENTATION: &str =
    "std/kernel-file-copy-result-presentation@1";

pub fn copy_file_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::copy_file_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(COPY_FILE_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::COPY_FILE_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(COPY_FILE_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(COPY_FILE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(COPY_FILE_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(COPY_FILE_HOST_OPERATION_CONTRACT),
            target_kind: Some(kind_id(conduit_semantic_catalog::COPY_FILE_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: COPY_COMMAND_BYTES,
            maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: vec![
            protected_resource_requirement(
                conduit_semantic_catalog::COPY_DESTINATION_ROLE,
                conduit_semantic_catalog::PROTECTED_FILE_RESOURCE_CLASS,
                1,
            ),
            protected_resource_requirement(
                conduit_semantic_catalog::COPY_SOURCE_ROLE,
                conduit_semantic_catalog::PROTECTED_FILE_RESOURCE_CLASS,
                1,
            ),
        ],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

pub fn copy_result_presentation_offer() -> CapabilityOffer {
    let value_kind = conduit_semantic_catalog::copy_result_type()
        .profile()
        .expect("checked copy result type has a finite profile")
        .value_kind()
        .clone();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("std-file-copy-result-presentation"),
        kind_id: kind_id(conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND),
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::STRUCTURED_PRESENTATION_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(COPY_FILE_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(COPY_RESULT_PRESENTATION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(COPY_FILE_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind,
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: vec![present_host_operation_requirement(
            kind_id(conduit_semantic_catalog::STRUCTURED_PRESENTATION_TARGET),
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        )],
        resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_offer_requires_two_named_protected_files_and_one_bounded_step() {
        let offer = copy_file_offer();
        assert!(offer.inputs.is_empty());
        assert_eq!(offer.outputs.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 2);
        assert_eq!(
            offer.resource_requirements[0]
                .protected_role
                .as_ref()
                .map(|role| role.as_str()),
            Some(conduit_semantic_catalog::COPY_DESTINATION_ROLE)
        );
        assert_eq!(
            offer.resource_requirements[1]
                .protected_role
                .as_ref()
                .map(|role| role.as_str()),
            Some(conduit_semantic_catalog::COPY_SOURCE_ROLE)
        );
        assert_eq!(offer.host_operations[0].maximum_input_bytes, 1);
        assert_eq!(
            offer.host_operations[0].maximum_output_bytes,
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        );
    }
}
