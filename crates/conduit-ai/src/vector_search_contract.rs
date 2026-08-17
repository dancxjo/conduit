//! Portable vector-search face and deterministic exact-oracle realization facts.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, FaceStartupParameter, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision, KindId,
    PortDescriptor, PortDirection, PortTemporal,
};
use serde::{Deserialize, Serialize};

use crate::{MAXIMUM_SIMILARITY_TOP_K, MAXIMUM_VECTOR_INDEX_QUERY_WORK_UNITS};

pub const VECTOR_SEARCH_KIND: &str = "retrieval/vector-search";
pub const VECTOR_SEARCH_REVISION: &str = "conduit.ai/vector-search@1";
pub const SIMILARITY_QUERY_VALUE_KIND: &str = "retrieval/similarity-query@1";
pub const SIMILARITY_HITS_VALUE_KIND: &str = "retrieval/similarity-hits@1";
pub const VECTOR_SEARCH_OPERATION: &str = "conduit.host/vector-search@1";
pub const VECTOR_SEARCH_RESOURCE_CLASS: &str = crate::VECTOR_INDEX_RESOURCE_CLASS;
pub const MAXIMUM_VECTOR_SEARCH_INPUT_BYTES: u32 = 262_144;
pub const MAXIMUM_VECTOR_SEARCH_OUTPUT_BYTES: u32 = 1_048_576;
pub const EXACT_VECTOR_SEARCH_EXECUTION_PROFILE: &str = "conduit.vector-search/exact-oracle@1";
pub const EXACT_VECTOR_SEARCH_IMPLEMENTATION: &str = "portable/vector-exact-search@1";
pub const EXACT_VECTOR_SEARCH_ARTIFACT: &str = "conduit-ai/vector-exact-search@1";
pub const MAXIMUM_VECTOR_SEARCH_PROCESS_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorSearchContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
    pub maximum_query_work_units: u32,
    pub maximum_results: u32,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorSearchOfferInvalidity {
    EmptyProcessIdentity,
    ProcessIdentityTooLarge,
}

pub fn vector_search_contract() -> VectorSearchContract {
    VectorSearchContract {
        kind_id: kind_id(VECTOR_SEARCH_KIND),
        kind_contract_revision: KindContractRevision::from(VECTOR_SEARCH_REVISION),
        inputs: vec![port(
            "query",
            SIMILARITY_QUERY_VALUE_KIND,
            PortDirection::Input,
        )],
        outputs: vec![port(
            "hits",
            SIMILARITY_HITS_VALUE_KIND,
            PortDirection::Output,
        )],
        maximum_input_bytes: MAXIMUM_VECTOR_SEARCH_INPUT_BYTES,
        maximum_output_bytes: MAXIMUM_VECTOR_SEARCH_OUTPUT_BYTES,
        maximum_query_work_units: MAXIMUM_VECTOR_INDEX_QUERY_WORK_UNITS,
        maximum_results: MAXIMUM_SIMILARITY_TOP_K,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_VECTOR_SEARCH_INPUT_BYTES,
        },
    }
}

pub fn vector_search_startup_parameters() -> Vec<FaceStartupParameter> {
    [
        "maximum-input-bytes",
        "maximum-output-bytes",
        "maximum-query-work-units",
        "maximum-results",
    ]
    .into_iter()
    .map(|name| FaceStartupParameter {
        name: name.into(),
        value_type: "Count".into(),
        has_default: true,
    })
    .collect()
}

pub fn exact_vector_search_offer(
    process_identity: &str,
) -> Result<CapabilityOffer, VectorSearchOfferInvalidity> {
    validate_process_identity(process_identity)?;
    let contract = vector_search_contract();
    Ok(CapabilityOffer {
        startup_parameters: vector_search_startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!(
            "vector-search/exact/process/{process_identity}"
        )),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision.clone(),
        inputs: contract.inputs.clone(),
        outputs: contract.outputs.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(EXACT_VECTOR_SEARCH_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(EXACT_VECTOR_SEARCH_IMPLEMENTATION),
            artifact_id: ArtifactId::from(EXACT_VECTOR_SEARCH_ARTIFACT),
        },
        host_operations: vec![vector_search_operation(&contract)],
        resource_requirements: vec![resource_requirement(
            VECTOR_SEARCH_RESOURCE_CLASS,
            contract.maximum_query_work_units,
        )],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    })
}

pub fn vector_search_operation(contract: &VectorSearchContract) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(VECTOR_SEARCH_OPERATION),
        target_kind: Some(contract.kind_id.clone()),
        maximum_in_flight: contract.limits.max_active_instances,
        maximum_input_bytes: contract.maximum_input_bytes,
        maximum_output_bytes: contract.maximum_output_bytes,
    }
}

pub fn validate_process_identity(
    process_identity: &str,
) -> Result<(), VectorSearchOfferInvalidity> {
    if process_identity.is_empty() {
        return Err(VectorSearchOfferInvalidity::EmptyProcessIdentity);
    }
    if process_identity.len() > MAXIMUM_VECTOR_SEARCH_PROCESS_IDENTITY_BYTES {
        return Err(VectorSearchOfferInvalidity::ProcessIdentityTooLarge);
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
pub fn install_vector_search_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    let contract = vector_search_contract();
    let parameters = [
        (
            "maximum-input-bytes",
            u64::from(contract.maximum_input_bytes),
        ),
        (
            "maximum-output-bytes",
            u64::from(contract.maximum_output_bytes),
        ),
        (
            "maximum-query-work-units",
            u64::from(contract.maximum_query_work_units),
        ),
        ("maximum-results", u64::from(contract.maximum_results)),
    ];
    startup.insert(KindSignature {
        kind: VECTOR_SEARCH_KIND.to_string(),
        startup_parameters: parameters
            .iter()
            .map(|(name, maximum)| StartupParameterSignature {
                name: (*name).to_string(),
                value_type: "Count".to_string(),
                default: Some(maximum.to_string()),
            })
            .collect(),
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: parameters
                .into_iter()
                .map(|(key, maximum)| ConfigurationField {
                    key: key.to_string(),
                    default_value: conduit_core::ConfigurationValue::U64(maximum),
                    validation: ConfigurationRule::U64Range {
                        minimum: 1,
                        maximum,
                    },
                })
                .collect(),
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_contract_is_typed_finite_and_backend_neutral() {
        let contract = vector_search_contract();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            SIMILARITY_QUERY_VALUE_KIND
        );
        assert_eq!(
            contract.outputs[0].value_kind.as_str(),
            SIMILARITY_HITS_VALUE_KIND
        );
        assert!(contract.maximum_query_work_units > 0);
        assert!(contract.maximum_results > 0);
        let encoded = serde_json::to_string(&contract).expect("contract serializes");
        for forbidden in [
            "hnsw",
            "instant-distance",
            "database",
            "provider",
            "seed",
            "ef_search",
        ] {
            assert!(!encoded.to_ascii_lowercase().contains(forbidden));
        }
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn ordinary_form_checks_and_expands_without_backend_vocabulary() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_vector_search_catalog(&mut startup, &mut profile).expect("catalog installs");
        let source = "form retrieval {\n search: retrieval/vector-search(4096, 8192, 1024, 8)\n}\n";
        let checked = conduit_form::check_syntax_document(
            &conduit_form::parse_syntax_document(source),
            &startup,
        )
        .expect("portable syntax checks");
        let expanded = conduit_form::expand_canonical_form(&checked, "retrieval", &profile)
            .expect("portable form expands");
        assert_eq!(expanded.gears[0].kind_id.as_str(), VECTOR_SEARCH_KIND);
        assert_eq!(expanded.gears[0].configuration.len(), 4);
        for forbidden in [
            "hnsw",
            "instant-distance",
            "database",
            "provider",
            "seed",
            "host",
        ] {
            assert!(!source.contains(forbidden));
        }
    }
}
