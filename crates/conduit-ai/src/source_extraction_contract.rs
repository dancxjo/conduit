//! Portable bounded source-extraction face and deterministic realization offer.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, protected_resource_requirement, ArtifactId, AuthorityContractId,
    AuthorityRequirement, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, KindId, PortDescriptor, PortDirection, PortTemporal,
};
use serde::{Deserialize, Serialize};

pub const SOURCE_EXTRACTION_KIND: &str = "retrieval/extract-source";
pub const SOURCE_EXTRACTION_REVISION: &str = "conduit.ai/extract-source@1";
pub const SOURCE_REFERENCE_VALUE_KIND: &str = conduit_core::RESOURCE_REFERENCE_INFO_ID;
pub const SOURCE_CHUNKS_VALUE_KIND: &str = "retrieval/source-chunks@1";
pub const SOURCE_EXTRACTION_OPERATION: &str = "conduit.host/extract-source@1";
pub const SOURCE_READER_RESOURCE_CLASS: &str = "conduit.resource/bounded-source-reader@1";
pub const SOURCE_READER_RESOURCE_ROLE: &str = "source";
pub const SOURCE_READ_AUTHORITY: &str = "conduit.authority/read-bounded-source@1";
pub const DETERMINISTIC_EXTRACTION_EXECUTION_PROFILE: &str =
    "conduit.source-extraction/deterministic@1";
pub const DETERMINISTIC_EXTRACTION_IMPLEMENTATION: &str =
    "portable/deterministic-source-extraction@1";
pub const DETERMINISTIC_EXTRACTION_ARTIFACT: &str = "conduit-ai/deterministic-source-extraction@1";
pub const MAXIMUM_EXTRACTION_SOURCE_BYTES: u32 = 1_048_576;
pub const MAXIMUM_EXTRACTION_OUTPUT_BYTES: u32 = 1_048_576;
pub const MAXIMUM_EXTRACTION_CHUNK_BYTES: u32 = 65_536;
pub const MAXIMUM_EXTRACTION_CHUNKS: u32 = 1_024;
pub const MAXIMUM_EXTRACTION_WORK_UNITS: u32 = 2_097_152;
pub const MAXIMUM_EXTRACTION_PROCESS_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceExtractionContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub maximum_source_bytes: u32,
    pub maximum_output_bytes: u32,
    pub maximum_chunk_bytes: u32,
    pub maximum_chunks: u32,
    pub maximum_work_units: u32,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceExtractionOfferInvalidity {
    EmptyProcessIdentity,
    ProcessIdentityTooLarge,
}

pub fn source_extraction_contract() -> SourceExtractionContract {
    SourceExtractionContract {
        kind_id: kind_id(SOURCE_EXTRACTION_KIND),
        kind_contract_revision: KindContractRevision::from(SOURCE_EXTRACTION_REVISION),
        inputs: vec![port(
            "source",
            SOURCE_REFERENCE_VALUE_KIND,
            PortDirection::Input,
        )],
        outputs: vec![port(
            "chunks",
            SOURCE_CHUNKS_VALUE_KIND,
            PortDirection::Output,
        )],
        maximum_source_bytes: MAXIMUM_EXTRACTION_SOURCE_BYTES,
        maximum_output_bytes: MAXIMUM_EXTRACTION_OUTPUT_BYTES,
        maximum_chunk_bytes: MAXIMUM_EXTRACTION_CHUNK_BYTES,
        maximum_chunks: MAXIMUM_EXTRACTION_CHUNKS,
        maximum_work_units: MAXIMUM_EXTRACTION_WORK_UNITS,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_EXTRACTION_SOURCE_BYTES,
        },
    }
}

pub fn source_extraction_startup_parameters() -> Vec<FaceStartupParameter> {
    [
        "profile",
        "maximum-source-bytes",
        "maximum-output-bytes",
        "maximum-chunk-bytes",
        "maximum-chunks",
        "maximum-work-units",
    ]
    .into_iter()
    .map(|name| FaceStartupParameter {
        name: name.into(),
        value_type: if name == "profile" { "Text" } else { "Count" }.into(),
        has_default: true,
    })
    .collect()
}

pub fn deterministic_source_extraction_offer(
    process_identity: &str,
) -> Result<CapabilityOffer, SourceExtractionOfferInvalidity> {
    validate_process_identity(process_identity)?;
    let contract = source_extraction_contract();
    Ok(CapabilityOffer {
        startup_parameters: source_extraction_startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!(
            "source-extraction/deterministic/process/{process_identity}"
        )),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision.clone(),
        inputs: contract.inputs.clone(),
        outputs: contract.outputs.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                DETERMINISTIC_EXTRACTION_EXECUTION_PROFILE,
            ),
            implementation_id: ImplementationId::from(DETERMINISTIC_EXTRACTION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(DETERMINISTIC_EXTRACTION_ARTIFACT),
        },
        host_operations: vec![source_extraction_operation(&contract)],
        resource_requirements: vec![protected_resource_requirement(
            SOURCE_READER_RESOURCE_ROLE,
            SOURCE_READER_RESOURCE_CLASS,
            contract.maximum_work_units,
        )],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(SOURCE_READ_AUTHORITY),
            host_operation_contract_id: HostOperationContractId::from(SOURCE_EXTRACTION_OPERATION),
            subject_kind: contract.kind_id,
        }],
        limits: contract.limits,
    })
}

pub fn source_extraction_operation(
    contract: &SourceExtractionContract,
) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(SOURCE_EXTRACTION_OPERATION),
        target_kind: Some(contract.kind_id.clone()),
        maximum_in_flight: contract.limits.max_active_instances,
        maximum_input_bytes: conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
        maximum_output_bytes: contract.maximum_output_bytes,
    }
}

fn validate_process_identity(
    process_identity: &str,
) -> Result<(), SourceExtractionOfferInvalidity> {
    if process_identity.is_empty() {
        return Err(SourceExtractionOfferInvalidity::EmptyProcessIdentity);
    }
    if process_identity.len() > MAXIMUM_EXTRACTION_PROCESS_IDENTITY_BYTES {
        return Err(SourceExtractionOfferInvalidity::ProcessIdentityTooLarge);
    }
    Ok(())
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_source_extraction_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    let contract = source_extraction_contract();
    startup.insert(KindSignature {
        kind: SOURCE_EXTRACTION_KIND.to_string(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "profile".into(),
                value_type: "Text".into(),
                default: Some("text-utf8".into()),
            },
            count_parameter("maximum-source-bytes", contract.maximum_source_bytes),
            count_parameter("maximum-output-bytes", contract.maximum_output_bytes),
            count_parameter("maximum-chunk-bytes", contract.maximum_chunk_bytes),
            count_parameter("maximum-chunks", contract.maximum_chunks),
            count_parameter("maximum-work-units", contract.maximum_work_units),
        ],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![
                ConfigurationField {
                    key: "profile".into(),
                    default_value: conduit_core::ConfigurationValue::Text("text-utf8".into()),
                    validation: ConfigurationRule::TextOneOf {
                        values: vec![
                            "text-utf8".into(),
                            "structured-items".into(),
                            "resource-metadata".into(),
                        ],
                    },
                },
                count_field("maximum-source-bytes", contract.maximum_source_bytes),
                count_field("maximum-output-bytes", contract.maximum_output_bytes),
                count_field("maximum-chunk-bytes", contract.maximum_chunk_bytes),
                count_field("maximum-chunks", contract.maximum_chunks),
                count_field("maximum-work-units", contract.maximum_work_units),
            ],
        })
        .map_err(|error| error.to_string())
}

#[cfg(feature = "form-catalog")]
fn count_parameter(name: &str, maximum: u32) -> conduit_form::StartupParameterSignature {
    use alloc::string::ToString;

    conduit_form::StartupParameterSignature {
        name: name.into(),
        value_type: "Count".into(),
        default: Some(maximum.to_string()),
    }
}

#[cfg(feature = "form-catalog")]
fn count_field(name: &str, maximum: u32) -> conduit_form::ConfigurationField {
    conduit_form::ConfigurationField {
        key: name.into(),
        default_value: conduit_core::ConfigurationValue::U64(u64::from(maximum)),
        validation: conduit_form::ConfigurationRule::U64Range {
            minimum: 1,
            maximum: u64::from(maximum),
        },
    }
}
