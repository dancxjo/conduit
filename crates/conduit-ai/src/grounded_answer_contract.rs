//! Portable provider-free `rag/answer` face and reviewed Plan configuration.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, ImplementationId, ImplementationOffer,
    KindContractRevision, KindId, PortDescriptor, PortDirection, PortTemporal,
};

pub const RAG_ANSWER_KIND: &str = "rag/answer";
pub const RAG_ANSWER_REVISION: &str = "conduit.ai/rag-answer@1";
pub const RETRIEVAL_QUERY_INTENT_VALUE_KIND: &str = "retrieval/query-intent@1";
pub const GROUNDED_ANSWER_VALUE_KIND: &str = "rag/grounded-answer@1";
pub const RAG_ANSWER_EXECUTION_PROFILE: &str = "conduit.rag/ordinary-model@1";
pub const RAG_ANSWER_IMPLEMENTATION: &str = "portable/grounded-answer@1";
pub const MAXIMUM_RAG_ANSWER_VALUE_BYTES: u32 = 1_048_576;
pub const MAXIMUM_RAG_ANSWER_PROCESS_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RagAnswerContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RagAnswerOfferInvalidity {
    EmptyProcessIdentity,
    ProcessIdentityTooLarge,
}

pub fn rag_answer_contract() -> RagAnswerContract {
    RagAnswerContract {
        kind_id: kind_id(RAG_ANSWER_KIND),
        kind_contract_revision: KindContractRevision::from(RAG_ANSWER_REVISION),
        inputs: vec![
            port(
                "query",
                RETRIEVAL_QUERY_INTENT_VALUE_KIND,
                PortDirection::Input,
            ),
            port(
                "context",
                crate::STRUCTURED_CONTEXT_VALUE_KIND,
                PortDirection::Input,
            ),
        ],
        outputs: vec![port(
            "answer",
            GROUNDED_ANSWER_VALUE_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_RAG_ANSWER_VALUE_BYTES,
        },
    }
}

pub fn ordinary_rag_answer_offer(
    process_identity: &str,
) -> Result<CapabilityOffer, RagAnswerOfferInvalidity> {
    if process_identity.is_empty() {
        return Err(RagAnswerOfferInvalidity::EmptyProcessIdentity);
    }
    if process_identity.len() > MAXIMUM_RAG_ANSWER_PROCESS_IDENTITY_BYTES {
        return Err(RagAnswerOfferInvalidity::ProcessIdentityTooLarge);
    }
    let contract = rag_answer_contract();
    Ok(CapabilityOffer {
        startup_parameters: startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!("rag/answer/process/{process_identity}")),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(RAG_ANSWER_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(RAG_ANSWER_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-ai/grounded-answer@1"),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: contract.limits,
    })
}

fn startup_parameters() -> Vec<FaceStartupParameter> {
    [
        ("policy", "Text"),
        ("answer-kind", "Text"),
        ("maximum-output-bytes", "Count"),
        ("maximum-claims", "Count"),
        ("maximum-citations", "Count"),
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

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_rag_answer_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_form::{KindDefinition, KindSignature, StartupParameterSignature};

    startup.insert(KindSignature {
        kind: RAG_ANSWER_KIND.to_string(),
        startup_parameters: vec![
            text_parameter("policy", "grounding/exact-context-citations@1"),
            text_parameter("answer-kind", "value/text-utf8@1"),
            count_parameter("maximum-output-bytes", 65_536),
            count_parameter("maximum-claims", 64),
            count_parameter("maximum-citations", 128),
            count_parameter("maximum-work-units", 1_000_000),
        ],
    })?;
    let contract = rag_answer_contract();
    let result = profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![
                text_choice(
                    "policy",
                    "grounding/exact-context-citations@1",
                    &["grounding/exact-context-citations@1"],
                ),
                text_choice(
                    "answer-kind",
                    "value/text-utf8@1",
                    &["value/text-utf8@1", "value/structured-answer@1"],
                ),
                count_field(
                    "maximum-output-bytes",
                    crate::MAXIMUM_GROUNDED_ANSWER_BYTES as u64,
                ),
                count_field("maximum-claims", crate::MAXIMUM_GROUNDED_CLAIMS as u64),
                count_field("maximum-citations", crate::MAXIMUM_CITATIONS as u64),
                count_field(
                    "maximum-work-units",
                    crate::MAXIMUM_GROUNDED_ANSWER_WORK_UNITS,
                ),
            ],
        })
        .map_err(|error| error.to_string());

    fn text_parameter(name: &str, default: &str) -> StartupParameterSignature {
        StartupParameterSignature {
            name: name.into(),
            value_type: "Text".into(),
            default: Some(default.into()),
        }
    }
    fn count_parameter(name: &str, default: u64) -> StartupParameterSignature {
        StartupParameterSignature {
            name: name.into(),
            value_type: "Count".into(),
            default: Some(default.to_string()),
        }
    }
    result
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
fn count_field(key: &str, maximum: u64) -> conduit_form::ConfigurationField {
    conduit_form::ConfigurationField {
        key: key.into(),
        default_value: conduit_core::ConfigurationValue::U64(maximum),
        validation: conduit_form::ConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    }
}
