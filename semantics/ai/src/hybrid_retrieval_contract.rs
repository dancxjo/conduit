//! Portable four-path hybrid retrieval face and deterministic fusion offer.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, ImplementationId, ImplementationOffer,
    KindContractRevision, KindId, PortDescriptor, PortDirection, PortTemporal,
};

pub const HYBRID_RETRIEVAL_KIND: &str = "retrieval/hybrid-fuse";
pub const HYBRID_RETRIEVAL_REVISION: &str = "conduit.ai/hybrid-fuse@1";
pub const RETRIEVAL_CANDIDATE_BATCH_VALUE_KIND: &str = "retrieval/candidate-batch@1";
pub const HYBRID_RETRIEVAL_CANDIDATES_VALUE_KIND: &str = "retrieval/candidates@1";
pub const DETERMINISTIC_HYBRID_EXECUTION_PROFILE: &str = "conduit.hybrid-fusion/deterministic@1";
pub const DETERMINISTIC_HYBRID_IMPLEMENTATION: &str = "portable/deterministic-hybrid-fusion@1";
pub const DETERMINISTIC_HYBRID_ARTIFACT: &str = "conduit-ai/deterministic-hybrid-fusion@1";
pub const MAXIMUM_HYBRID_BATCH_BYTES: u32 = 1_048_576;
pub const MAXIMUM_HYBRID_QUEUE_BYTES: u32 = 4 * MAXIMUM_HYBRID_BATCH_BYTES;
pub const MAXIMUM_HYBRID_PROCESS_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridRetrievalContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub maximum_candidates_per_stage: u16,
    pub maximum_output_candidates: u16,
    pub maximum_work_units: u32,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridRetrievalOfferInvalidity {
    EmptyProcessIdentity,
    ProcessIdentityTooLarge,
}

pub fn hybrid_retrieval_contract() -> HybridRetrievalContract {
    HybridRetrievalContract {
        kind_id: kind_id(HYBRID_RETRIEVAL_KIND),
        kind_contract_revision: KindContractRevision::from(HYBRID_RETRIEVAL_REVISION),
        inputs: ["vector", "lexical", "metadata", "temporal"]
            .into_iter()
            .map(|name| {
                port(
                    name,
                    RETRIEVAL_CANDIDATE_BATCH_VALUE_KIND,
                    PortDirection::Input,
                )
            })
            .collect(),
        outputs: vec![port(
            "candidates",
            HYBRID_RETRIEVAL_CANDIDATES_VALUE_KIND,
            PortDirection::Output,
        )],
        maximum_candidates_per_stage: crate::MAXIMUM_HYBRID_CANDIDATES_PER_STAGE,
        maximum_output_candidates: crate::MAXIMUM_HYBRID_OUTPUT_CANDIDATES,
        maximum_work_units: crate::MAXIMUM_HYBRID_WORK_UNITS,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MAXIMUM_HYBRID_QUEUE_BYTES,
        },
    }
}

pub fn hybrid_retrieval_startup_parameters() -> Vec<FaceStartupParameter> {
    [
        ("policy", "Text"),
        ("strategy", "Text"),
        ("rank-constant", "Count"),
        ("temporal-hard-filter", "Text"),
        ("maximum-candidates-per-stage", "Count"),
        ("maximum-output-candidates", "Count"),
        ("maximum-work-units", "Count"),
    ]
    .into_iter()
    .map(|(name, value_type)| FaceStartupParameter {
        name: name.into(),
        value_type: value_type.into(),
        has_default: true,
    })
    .collect()
}

pub fn deterministic_hybrid_retrieval_offer(
    process_identity: &str,
) -> Result<CapabilityOffer, HybridRetrievalOfferInvalidity> {
    if process_identity.is_empty() {
        return Err(HybridRetrievalOfferInvalidity::EmptyProcessIdentity);
    }
    if process_identity.len() > MAXIMUM_HYBRID_PROCESS_IDENTITY_BYTES {
        return Err(HybridRetrievalOfferInvalidity::ProcessIdentityTooLarge);
    }
    let contract = hybrid_retrieval_contract();
    Ok(CapabilityOffer {
        startup_parameters: hybrid_retrieval_startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!(
            "hybrid-fusion/deterministic/process/{process_identity}"
        )),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(DETERMINISTIC_HYBRID_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(DETERMINISTIC_HYBRID_IMPLEMENTATION),
            artifact_id: ArtifactId::from(DETERMINISTIC_HYBRID_ARTIFACT),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: contract.limits,
    })
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
pub fn install_hybrid_retrieval_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_form::{KindDefinition, KindSignature};

    let contract = hybrid_retrieval_contract();
    startup.insert(KindSignature {
        kind: HYBRID_RETRIEVAL_KIND.to_string(),
        startup_parameters: vec![
            text_parameter("policy", "fusion/reciprocal-rank@1"),
            text_parameter("strategy", "reciprocal-rank"),
            count_parameter("rank-constant", 60),
            text_parameter("temporal-hard-filter", "none"),
            count_parameter(
                "maximum-candidates-per-stage",
                u32::from(contract.maximum_candidates_per_stage),
            ),
            count_parameter(
                "maximum-output-candidates",
                u32::from(contract.maximum_output_candidates),
            ),
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
                text_choice(
                    "policy",
                    "fusion/reciprocal-rank@1",
                    &[
                        "fusion/reciprocal-rank@1",
                        "fusion/reciprocal-rank-origin@1",
                    ],
                ),
                text_choice("strategy", "reciprocal-rank", &["reciprocal-rank"]),
                count_field("rank-constant", 1, u64::from(u16::MAX)),
                text_choice(
                    "temporal-hard-filter",
                    "none",
                    &["none", "earliest", "latest", "created-duration"],
                ),
                count_field(
                    "maximum-candidates-per-stage",
                    1,
                    u64::from(contract.maximum_candidates_per_stage),
                ),
                count_field(
                    "maximum-output-candidates",
                    1,
                    u64::from(contract.maximum_output_candidates),
                ),
                count_field(
                    "maximum-work-units",
                    1,
                    u64::from(contract.maximum_work_units),
                ),
            ],
        })
        .map_err(|error| error.to_string())
}

#[cfg(feature = "form-catalog")]
fn text_parameter(name: &str, default: &str) -> conduit_form::StartupParameterSignature {
    conduit_form::StartupParameterSignature {
        name: name.into(),
        value_type: "Text".into(),
        default: Some(default.into()),
    }
}

#[cfg(feature = "form-catalog")]
fn count_parameter(name: &str, default: u32) -> conduit_form::StartupParameterSignature {
    use alloc::string::ToString;
    conduit_form::StartupParameterSignature {
        name: name.into(),
        value_type: "Count".into(),
        default: Some(default.to_string()),
    }
}

#[cfg(feature = "form-catalog")]
fn text_choice(key: &str, default: &str, values: &[&str]) -> conduit_form::ConfigurationField {
    conduit_form::ConfigurationField {
        key: key.into(),
        default_value: conduit_core::ConfigurationValue::Text(default.into()),
        validation: conduit_form::ConfigurationRule::TextOneOf {
            values: values.iter().map(|value| (*value).into()).collect(),
        },
    }
}

#[cfg(feature = "form-catalog")]
fn count_field(key: &str, minimum: u64, maximum: u64) -> conduit_form::ConfigurationField {
    conduit_form::ConfigurationField {
        key: key.into(),
        default_value: conduit_core::ConfigurationValue::U64(maximum),
        validation: conduit_form::ConfigurationRule::U64Range { minimum, maximum },
    }
}
