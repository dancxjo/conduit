use super::{StandardKindContract, TerminalBehavior};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, protected_resource_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, KindContractRevision,
};

pub const COPY_FILE_KIND: &str = "file/copy";
pub const COPY_FILE_CONTRACT_REVISION: &str = "conduit.std/file-copy@1";
pub const COPY_FILE_EXECUTION_PROFILE: &str = "conduit.std/file-copy-kernel-hosted@1";
pub const COPY_FILE_IMPLEMENTATION: &str = "std/kernel-file-copy@1";
pub const COPY_FILE_ARTIFACT: &str = "conduit-std-host/file-copy@1";
pub const COPY_FILE_CAPABILITY: &str = "file-copy-v1";
pub const COPY_FILE_HOST_OPERATION_CONTRACT: &str = "conduit.host/file-copy-step@1";
pub const PROTECTED_FILE_RESOURCE_CLASS: &str = "conduit.resource/protected-file@1";
pub const COPY_SOURCE_ROLE: &str = "source";
pub const COPY_DESTINATION_ROLE: &str = "destination";
pub const COPY_CHUNK_BYTES: u32 = 4_096;
pub const COPY_COMMAND_BYTES: u32 = 1;

pub fn copy_file_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(COPY_FILE_KIND),
        plain_name: "Copy a file".to_string(),
        summary: "Copy one protected source into one protected destination in bounded steps."
            .to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: COPY_COMMAND_BYTES,
        },
        terminal_behavior: TerminalBehavior::CompletesAfterFixedCount { count: 1 },
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "copy: file/copy".to_string(),
    }
}

pub fn copy_file_offer() -> CapabilityOffer {
    let contract = copy_file_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(COPY_FILE_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(COPY_FILE_CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(COPY_FILE_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(COPY_FILE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(COPY_FILE_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(COPY_FILE_HOST_OPERATION_CONTRACT),
            target_kind: Some(kind_id(COPY_FILE_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: COPY_COMMAND_BYTES,
            maximum_output_bytes: COPY_COMMAND_BYTES,
        }],
        resource_requirements: vec![
            protected_resource_requirement(COPY_DESTINATION_ROLE, PROTECTED_FILE_RESOURCE_CLASS, 1),
            protected_resource_requirement(COPY_SOURCE_ROLE, PROTECTED_FILE_RESOURCE_CLASS, 1),
        ],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_copy_file_catalog(catalog: &mut conduit_form::ProfileCatalog) -> Result<(), String> {
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(COPY_FILE_KIND),
            kind_contract_revision: KindContractRevision::from(COPY_FILE_CONTRACT_REVISION),
            inputs: Vec::new(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_offer_requires_two_named_protected_files_and_one_bounded_step() {
        let offer = copy_file_offer();
        assert!(offer.inputs.is_empty() && offer.outputs.is_empty());
        assert_eq!(offer.resource_requirements.len(), 2);
        assert_eq!(
            offer.resource_requirements[0]
                .protected_role
                .as_ref()
                .map(|role| role.as_str()),
            Some(COPY_DESTINATION_ROLE)
        );
        assert_eq!(
            offer.resource_requirements[1]
                .protected_role
                .as_ref()
                .map(|role| role.as_str()),
            Some(COPY_SOURCE_ROLE)
        );
        assert_eq!(offer.host_operations[0].maximum_input_bytes, 1);
        assert_eq!(offer.host_operations[0].maximum_output_bytes, 1);
    }
}
