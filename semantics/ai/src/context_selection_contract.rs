//! Portable R3 reranking and structured context-selection fronts.

use crate::R3OfferInvalidity;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FrontStartupParameter, ImplementationId, ImplementationOffer, Kind,
    KindConfigurationField, KindConfigurationRule, KindId, KindIdentity, PortDescriptor,
    PortDirection, PortTemporal,
};

pub const RERANK_KIND: &str = "retrieval/rerank";
pub const RERANK_REVISION: &str = "conduit.ai/retrieval-rerank@1";
pub const CONTEXT_SELECT_KIND: &str = "context/select";
pub const CONTEXT_SELECT_REVISION: &str = "conduit.ai/context-select@1";
pub const RERANKED_CANDIDATES_VALUE_KIND: &str = "retrieval/reranked-candidates@1";
pub const STRUCTURED_CONTEXT_VALUE_KIND: &str = "context/structured@1";
pub const R3_EXECUTION_PROFILE: &str = "conduit.r3/deterministic@1";
pub const RERANK_IMPLEMENTATION: &str = "portable/deterministic-rerank@1";
pub const CONTEXT_SELECT_IMPLEMENTATION: &str = "portable/structured-context-select@1";
pub const MAXIMUM_R3_VALUE_BYTES: u32 = 1_048_576;
pub const MAXIMUM_R3_PROCESS_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R3Contract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

pub fn rerank_contract() -> R3Contract {
    contract(
        RERANK_KIND,
        RERANK_REVISION,
        crate::HYBRID_RETRIEVAL_CANDIDATES_VALUE_KIND,
        RERANKED_CANDIDATES_VALUE_KIND,
    )
}

pub fn context_select_contract() -> R3Contract {
    contract(
        CONTEXT_SELECT_KIND,
        CONTEXT_SELECT_REVISION,
        RERANKED_CANDIDATES_VALUE_KIND,
        STRUCTURED_CONTEXT_VALUE_KIND,
    )
}

pub fn deterministic_rerank_offer(
    process_identity: &str,
) -> Result<CapabilityOffer, R3OfferInvalidity> {
    offer(
        process_identity,
        rerank_contract(),
        "r3/rerank",
        RERANK_IMPLEMENTATION,
        "conduit-ai/deterministic-rerank@1",
        rerank_startup_parameters(),
    )
}

pub fn deterministic_context_select_offer(
    process_identity: &str,
) -> Result<CapabilityOffer, R3OfferInvalidity> {
    offer(
        process_identity,
        context_select_contract(),
        "r3/context-select",
        CONTEXT_SELECT_IMPLEMENTATION,
        "conduit-ai/structured-context-select@1",
        context_startup_parameters(),
    )
}

fn contract(kind: &str, revision: &str, input: &str, output: &str) -> R3Contract {
    R3Contract {
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        inputs: vec![port("candidates", input, PortDirection::Input)],
        outputs: vec![port("result", output, PortDirection::Output)],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_R3_VALUE_BYTES,
        },
    }
}

fn offer(
    process_identity: &str,
    contract: R3Contract,
    capability_prefix: &str,
    implementation: &str,
    artifact: &str,
    startup_parameters: Vec<FrontStartupParameter>,
) -> Result<CapabilityOffer, R3OfferInvalidity> {
    if process_identity.is_empty() {
        return Err(R3OfferInvalidity::EmptyProcessIdentity);
    }
    if process_identity.len() > MAXIMUM_R3_PROCESS_IDENTITY_BYTES {
        return Err(R3OfferInvalidity::ProcessIdentityTooLarge);
    }
    let semantic_contract = match contract.kind_id.as_str() {
        RERANK_KIND => rerank_semantic_contract().semantic_contract(),
        CONTEXT_SELECT_KIND => context_select_semantic_contract().semantic_contract(),
        _ => unreachable!("the R3 offer family is closed"),
    };
    Ok(conduit_core::capability_offer_from_parts! {
        startup_parameters,
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!(
            "{capability_prefix}/process/{process_identity}"
        )),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        semantic_contract,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(R3_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: contract.limits,
    })
}

fn rerank_semantic_contract() -> Kind {
    canonical_kind(
        rerank_contract(),
        rerank_startup_parameters(),
        vec![
            text_choice(
                "policy",
                "rerank/preserve-hybrid-deterministic@1",
                &[
                    "rerank/preserve-hybrid-deterministic@1",
                    "rerank/observed-model-derived@1",
                ],
            ),
            count_field(
                "maximum-candidates",
                crate::MAXIMUM_HYBRID_OUTPUT_CANDIDATES.into(),
            ),
            count_field(
                "maximum-work-units",
                crate::MAXIMUM_RERANKING_WORK_UNITS.into(),
            ),
        ],
    )
}

fn context_select_semantic_contract() -> Kind {
    canonical_kind(
        context_select_contract(),
        context_startup_parameters(),
        vec![
            text_choice(
                "policy",
                "context/reranked-diverse@1",
                &[
                    "context/reranked-diverse@1",
                    "context/chronological-diverse@1",
                ],
            ),
            text_choice(
                "token-accounting-profile",
                "tokens/reviewed@1",
                &["tokens/reviewed@1", "tokens/exact-fixture@1"],
            ),
            text_choice(
                "redundancy",
                "one-per-reviewed-group",
                &["keep-all", "one-per-reviewed-group"],
            ),
            text_choice(
                "ordering",
                "reranked",
                &["reranked", "chronological-oldest-first"],
            ),
            count_field("maximum-items", crate::MAXIMUM_CONTEXT_ITEMS as u64),
            count_field("maximum-bytes", crate::MAXIMUM_CONTEXT_BYTES.into()),
            count_field(
                "maximum-tokens",
                crate::MAXIMUM_CONTEXT_SELECTION_TOKENS.into(),
            ),
            count_field(
                "maximum-work-units",
                crate::MAXIMUM_CONTEXT_SELECTION_WORK_UNITS.into(),
            ),
        ],
    )
}

fn canonical_kind(
    contract: R3Contract,
    startup_parameters: Vec<FrontStartupParameter>,
    configuration: Vec<KindConfigurationField>,
) -> Kind {
    Kind {
        startup_parameters,
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration,
        semantic_laws: Vec::new(),
        limits: contract.limits,
    }
}

fn rerank_startup_parameters() -> Vec<FrontStartupParameter> {
    [
        ("policy", "Text"),
        ("maximum-candidates", "Count"),
        ("maximum-work-units", "Count"),
    ]
    .into_iter()
    .map(parameter)
    .collect()
}

fn context_startup_parameters() -> Vec<FrontStartupParameter> {
    [
        ("policy", "Text"),
        ("token-accounting-profile", "Text"),
        ("redundancy", "Text"),
        ("ordering", "Text"),
        ("maximum-items", "Count"),
        ("maximum-bytes", "Count"),
        ("maximum-tokens", "Count"),
        ("maximum-work-units", "Count"),
    ]
    .into_iter()
    .map(parameter)
    .collect()
}

fn parameter((name, value_type): (&str, &str)) -> FrontStartupParameter {
    FrontStartupParameter {
        name: name.into(),
        value_type: kind_id(match value_type {
            "Text" => "value/text",
            "Count" => "value/count",
            exact => exact,
        }),
        has_default: true,
    }
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_r3_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_plot::KindSignature;

    startup.insert(KindSignature {
        kind: RERANK_KIND.to_string(),
        startup_parameters: vec![
            text_parameter("policy", "rerank/preserve-hybrid-deterministic@1"),
            count_parameter("maximum-candidates", 64),
            count_parameter("maximum-work-units", 65_536),
        ],
    })?;
    startup.insert(KindSignature {
        kind: CONTEXT_SELECT_KIND.to_string(),
        startup_parameters: vec![
            text_parameter("policy", "context/reranked-diverse@1"),
            text_parameter("token-accounting-profile", "tokens/reviewed@1"),
            text_parameter("redundancy", "one-per-reviewed-group"),
            text_parameter("ordering", "reranked"),
            count_parameter("maximum-items", 32),
            count_parameter("maximum-bytes", 65_536),
            count_parameter("maximum-tokens", 16_384),
            count_parameter("maximum-work-units", 65_536),
        ],
    })?;
    profile
        .insert_kind(rerank_semantic_contract())
        .map_err(|error| error.to_string())?;
    profile
        .insert_kind(context_select_semantic_contract())
        .map_err(|error| error.to_string())
}

#[cfg(feature = "plot-catalog")]
fn text_parameter(name: &str, default: &str) -> conduit_plot::StartupParameterSignature {
    conduit_plot::StartupParameterSignature {
        name: name.into(),
        value_type: "Text".into(),
        default: Some(default.into()),
    }
}

#[cfg(feature = "plot-catalog")]
fn count_parameter(name: &str, default: u32) -> conduit_plot::StartupParameterSignature {
    use alloc::string::ToString;
    conduit_plot::StartupParameterSignature {
        name: name.into(),
        value_type: "Count".into(),
        default: Some(default.to_string()),
    }
}

fn text_choice(key: &str, default: &str, values: &[&str]) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: conduit_core::ConfigurationValue::Text(default.into()),
        rule: KindConfigurationRule::TextOneOf {
            values: values.iter().map(|value| (*value).into()).collect(),
        },
    }
}

fn count_field(key: &str, maximum: u64) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: conduit_core::ConfigurationValue::U64(maximum),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    }
}
