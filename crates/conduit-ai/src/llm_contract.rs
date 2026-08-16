use alloc::{format, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal,
};
use serde::{Deserialize, Serialize};

pub const LLM_GENERATE_KIND: &str = "llm/generate";
pub const LLM_CLASSIFY_KIND: &str = "llm/classify";
pub const LLM_EXTRACT_KIND: &str = "llm/extract";
pub const LLM_EMBED_KIND: &str = "llm/embed";
pub const LLM_INTERPRET_KIND: &str = "llm/interpret";
pub const LLM_PROPOSE_KIND: &str = "llm/propose";
pub const LLM_COMPOSE_KIND: &str = "llm/compose";
pub const LLM_JUDGE_KIND: &str = "llm/judge";

pub const MAXIMUM_LLM_INPUT_BYTES: u64 = 262_144;
pub const MAXIMUM_LLM_CONTEXT_ITEMS: u64 = 128;
pub const MAXIMUM_LLM_OUTPUT_BYTES: u64 = 65_536;
pub const MAXIMUM_LLM_WORK_UNITS: u64 = 1_000_000;
pub const MAXIMUM_LLM_HISTORY_ITEMS: u64 = 64;
pub const MAXIMUM_LLM_CATALOG_KINDS: usize = 8;

const GENERATION_REQUEST: &str = "llm/generation-request@1";
const GENERATED_RESULT: &str = "llm/generated-result@1";
const CLASSIFICATION_REQUEST: &str = "llm/classification-request@1";
const CLASSIFICATION_RESULT: &str = "llm/classification-result@1";
const EXTRACTION_REQUEST: &str = "llm/extraction-request@1";
const EXTRACTION_RESULT: &str = "llm/extraction-result@1";
const EMBEDDING_REQUEST: &str = "llm/embedding-request@1";
const EMBEDDING_RESULT: &str = "llm/embedding-result@1";
const INTERPRETATION_REQUEST: &str = "llm/interpretation-request@1";
const INTERPRETATION_RESULT: &str = "llm/interpretation-result@1";
const PROPOSAL_REQUEST: &str = "llm/proposal-request@1";
const PROPOSAL_RESULT: &str = "llm/proposal-result@1";
const COMPOSITION_REQUEST: &str = "llm/composition-request@1";
const COMPOSITION_RESULT: &str = "llm/composition-result@1";
const JUDGMENT_REQUEST: &str = "llm/judgment-request@1";
const JUDGMENT_RESULT: &str = "llm/judgment-result@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmDeterminismProfile {
    /// Exact output equality is valid only for the catalog's pure validation fixtures.
    DeterministicValidationFixture,
    /// A seed is implementation input and does not establish universal equality.
    SeededImplementationBestEffort,
    StochasticInference,
    ProviderNondeterministic,
}

impl LlmDeterminismProfile {
    pub const fn permits_semantic_output_equality_claim(self) -> bool {
        matches!(self, Self::DeterministicValidationFixture)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmTerminalOutcome {
    Produced,
    Truncated,
    Refused,
    Failed,
    Cancelled,
    ProviderLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmImplementationControl {
    Temperature,
    Seed,
    Sampler,
    Quantization,
    PromptTemplate,
    ChatRoleEncoding,
    ProviderFunctionJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmWorkBounds {
    pub maximum_input_bytes: u64,
    pub maximum_context_items: u64,
    pub maximum_output_bytes: u64,
    pub maximum_work_units: u64,
    pub maximum_history_items: u64,
}

impl LlmWorkBounds {
    pub const fn reviewed_default() -> Self {
        Self {
            maximum_input_bytes: MAXIMUM_LLM_INPUT_BYTES,
            maximum_context_items: MAXIMUM_LLM_CONTEXT_ITEMS,
            maximum_output_bytes: MAXIMUM_LLM_OUTPUT_BYTES,
            maximum_work_units: MAXIMUM_LLM_WORK_UNITS,
            maximum_history_items: MAXIMUM_LLM_HISTORY_ITEMS,
        }
    }

    pub const fn valid(self) -> bool {
        self.maximum_input_bytes > 0
            && self.maximum_input_bytes <= MAXIMUM_LLM_INPUT_BYTES
            && self.maximum_context_items <= MAXIMUM_LLM_CONTEXT_ITEMS
            && self.maximum_output_bytes > 0
            && self.maximum_output_bytes <= MAXIMUM_LLM_OUTPUT_BYTES
            && self.maximum_work_units > 0
            && self.maximum_work_units <= MAXIMUM_LLM_WORK_UNITS
            && self.maximum_history_items <= MAXIMUM_LLM_HISTORY_ITEMS
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmSemanticContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub result_payload_kind: KindId,
    pub bounds: LlmWorkBounds,
    pub terminal_outcomes: [LlmTerminalOutcome; 6],
    pub excluded_implementation_controls: [LlmImplementationControl; 7],
    pub limits: CapabilityLimits,
}

impl LlmSemanticContract {
    pub fn is_exactly_compatible_with(&self, offered: &Self) -> bool {
        self.kind_id == offered.kind_id
            && self.kind_contract_revision == offered.kind_contract_revision
            && self.inputs == offered.inputs
            && self.outputs == offered.outputs
            && self.result_payload_kind == offered.result_payload_kind
            && self.bounds == offered.bounds
    }
}

pub fn llm_semantic_catalog() -> [LlmSemanticContract; MAXIMUM_LLM_CATALOG_KINDS] {
    [
        contract(LLM_GENERATE_KIND, GENERATION_REQUEST, GENERATED_RESULT),
        contract(
            LLM_CLASSIFY_KIND,
            CLASSIFICATION_REQUEST,
            CLASSIFICATION_RESULT,
        ),
        contract(LLM_EXTRACT_KIND, EXTRACTION_REQUEST, EXTRACTION_RESULT),
        contract(LLM_EMBED_KIND, EMBEDDING_REQUEST, EMBEDDING_RESULT),
        contract(
            LLM_INTERPRET_KIND,
            INTERPRETATION_REQUEST,
            INTERPRETATION_RESULT,
        ),
        contract(LLM_PROPOSE_KIND, PROPOSAL_REQUEST, PROPOSAL_RESULT),
        contract(LLM_COMPOSE_KIND, COMPOSITION_REQUEST, COMPOSITION_RESULT),
        contract(LLM_JUDGE_KIND, JUDGMENT_REQUEST, JUDGMENT_RESULT),
    ]
}

pub fn llm_contract(kind: &str) -> Option<LlmSemanticContract> {
    llm_semantic_catalog()
        .into_iter()
        .find(|contract| contract.kind_id.as_str() == kind)
}

fn contract(kind: &str, request_kind: &str, result_kind: &str) -> LlmSemanticContract {
    let bounds = LlmWorkBounds::reviewed_default();
    LlmSemanticContract {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("conduit.{kind}@1")),
        inputs: vec![port("request", request_kind, PortDirection::Input)],
        outputs: vec![port("result", result_kind, PortDirection::Output)],
        result_payload_kind: kind_id(result_kind),
        bounds,
        terminal_outcomes: [
            LlmTerminalOutcome::Produced,
            LlmTerminalOutcome::Truncated,
            LlmTerminalOutcome::Refused,
            LlmTerminalOutcome::Failed,
            LlmTerminalOutcome::Cancelled,
            LlmTerminalOutcome::ProviderLost,
        ],
        excluded_implementation_controls: [
            LlmImplementationControl::Temperature,
            LlmImplementationControl::Seed,
            LlmImplementationControl::Sampler,
            LlmImplementationControl::Quantization,
            LlmImplementationControl::PromptTemplate,
            LlmImplementationControl::ChatRoleEncoding,
            LlmImplementationControl::ProviderFunctionJson,
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: (bounds.maximum_input_bytes + bounds.maximum_output_bytes) as u32,
        },
    }
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
pub fn install_llm_semantic_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    for contract in llm_semantic_catalog() {
        let kind = contract.kind_id.as_str().to_string();
        let bounds = contract.bounds;
        startup.insert(KindSignature {
            kind: kind.clone(),
            startup_parameters: vec![
                parameter("maximum-input-bytes", bounds.maximum_input_bytes),
                parameter("maximum-context-items", bounds.maximum_context_items),
                parameter("maximum-output-bytes", bounds.maximum_output_bytes),
                parameter("maximum-work-units", bounds.maximum_work_units),
                parameter("maximum-history-items", bounds.maximum_history_items),
            ],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![
                    bound("maximum-input-bytes", bounds.maximum_input_bytes),
                    bound("maximum-context-items", bounds.maximum_context_items),
                    bound("maximum-output-bytes", bounds.maximum_output_bytes),
                    bound("maximum-work-units", bounds.maximum_work_units),
                    bound("maximum-history-items", bounds.maximum_history_items),
                ],
            })
            .map_err(|error| error.to_string())?;
    }

    fn bound(key: &str, maximum: u64) -> ConfigurationField {
        ConfigurationField {
            key: key.into(),
            default_value: conduit_core::ConfigurationValue::U64(maximum),
            validation: ConfigurationRule::U64Range {
                minimum: 0,
                maximum,
            },
        }
    }
    fn parameter(name: &str, default: u64) -> StartupParameterSignature {
        StartupParameterSignature {
            name: name.into(),
            value_type: "Count".into(),
            default: Some(default.to_string()),
        }
    }
    Ok(())
}
