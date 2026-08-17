use super::{StandardKindContract, TerminalBehavior};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, protected_resource_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, StructuredVariantCase,
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
pub const COPY_RESULT_TYPE: &str = "FileCopyResult";
pub const COPY_RESULT_PRESENTATION_IMPLEMENTATION: &str =
    "std/kernel-file-copy-result-presentation@1";

pub fn copy_result_type() -> StructuredInfoType {
    let quantity = StructuredInfoType::leaf(kind_id(conduit_core::QUANTITY_INFO_ID)).unwrap();
    let unit = StructuredInfoType::leaf(kind_id("value/unit@1")).unwrap();
    let outcome = StructuredInfoType::variant(
        kind_id("file/copy-outcome@1"),
        vec![
            StructuredVariantCase::new("cancelled", quantity.clone()).unwrap(),
            StructuredVariantCase::new("cleanup_failed", quantity.clone()).unwrap(),
            StructuredVariantCase::new("denied", unit.clone()).unwrap(),
            StructuredVariantCase::new("destination_exists", unit.clone()).unwrap(),
            StructuredVariantCase::new("oversized", quantity.clone()).unwrap(),
            StructuredVariantCase::new("partial", quantity.clone()).unwrap(),
            StructuredVariantCase::new("stale", unit).unwrap(),
            StructuredVariantCase::new("success", quantity).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoType::record(
        kind_id("file/copy-result@1"),
        vec![conduit_core::StructuredFieldType::new("outcome", outcome).unwrap()],
    )
    .unwrap()
}

pub fn copy_success_value(bytes_copied: u64) -> Result<conduit_core::StructuredInfoValue, String> {
    let bytes_copied = i64::try_from(bytes_copied)
        .map_err(|_| "copied byte count exceeds the quantity profile".to_string())?;
    let quantity_type = StructuredInfoType::leaf(kind_id(conduit_core::QUANTITY_INFO_ID)).unwrap();
    let quantity = conduit_core::StructuredInfoValue::leaf(
        quantity_type,
        conduit_core::Quantity::new(bytes_copied, conduit_core::QuantityUnit::Byte)
            .encode()
            .to_vec(),
    )
    .unwrap();
    let outcome_type = match copy_result_type().shape() {
        conduit_core::StructuredInfoTypeShape::Record { fields, .. } => {
            fields[0].value_type().clone()
        }
        _ => unreachable!(),
    };
    let outcome =
        conduit_core::StructuredInfoValue::variant(outcome_type, "success", quantity).unwrap();
    conduit_core::StructuredInfoValue::record(
        copy_result_type(),
        vec![conduit_core::StructuredFieldValue::new("outcome", outcome).unwrap()],
    )
    .map_err(|error| format!("encode file copy result: {error:?}"))
}

fn result_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: conduit_core::port_id(if direction == PortDirection::Output {
            "result"
        } else {
            "input"
        }),
        value_kind: copy_result_type().profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

pub fn copy_file_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(COPY_FILE_KIND),
        plain_name: "Copy a file".to_string(),
        summary: "Copy one protected source into one protected destination in bounded steps."
            .to_string(),
        inputs: Vec::new(),
        outputs: vec![result_port(PortDirection::Output)],
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
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
            maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: vec![
            protected_resource_requirement(COPY_DESTINATION_ROLE, PROTECTED_FILE_RESOURCE_CLASS, 1),
            protected_resource_requirement(COPY_SOURCE_ROLE, PROTECTED_FILE_RESOURCE_CLASS, 1),
        ],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

pub fn copy_result_presentation_offer() -> CapabilityOffer {
    let mut offer = crate::structured_presentation_std_offer(COPY_RESULT_TYPE, &copy_result_type());
    offer.capability_id = CapabilityId::from("std-file-copy-result-presentation");
    offer.implementation.execution_profile_id =
        ExecutionProfileId::from(COPY_FILE_EXECUTION_PROFILE);
    offer.implementation.implementation_id =
        ImplementationId::from(COPY_RESULT_PRESENTATION_IMPLEMENTATION);
    offer.implementation.artifact_id = ArtifactId::from(COPY_FILE_ARTIFACT);
    offer
}

#[cfg(feature = "form-catalog")]
pub fn install_copy_file_catalog(catalog: &mut conduit_form::ProfileCatalog) -> Result<(), String> {
    for definition in [
        conduit_form::KindDefinition {
            kind_id: kind_id(COPY_FILE_KIND),
            kind_contract_revision: KindContractRevision::from(COPY_FILE_CONTRACT_REVISION),
            inputs: Vec::new(),
            outputs: vec![result_port(PortDirection::Output)],
            configuration: Vec::new(),
        },
        conduit_form::KindDefinition {
            kind_id: kind_id(crate::STRUCTURED_PRESENTATION_KIND),
            kind_contract_revision: KindContractRevision::from(
                crate::STRUCTURED_PRESENTATION_REVISION,
            ),
            inputs: vec![result_port(PortDirection::Input)],
            outputs: Vec::new(),
            configuration: Vec::new(),
        },
    ] {
        catalog
            .insert(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
        assert_eq!(
            offer.host_operations[0].maximum_output_bytes,
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        );
    }
}
