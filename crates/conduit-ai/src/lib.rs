#![no_std]

extern crate alloc;

mod bases;
pub use bases::*;
mod llm_contract;
pub use llm_contract::*;
mod local_model;
pub use local_model::*;
mod interpretation;
pub use interpretation::*;
mod model_result;
pub use model_result::*;
mod structured_result;
pub use structured_result::*;
mod temporal_context;
pub use temporal_context::*;
mod temporal_evidence_selection;
pub use temporal_evidence_selection::*;
mod vector_retrieval;
pub use vector_retrieval::*;
#[cfg(feature = "form-catalog")]
mod provider;
#[cfg(feature = "form-catalog")]
pub use provider::*;

use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal,
};
use serde::{Deserialize, Serialize};

pub const GENERATE_TEXT_KIND: &str = "ai/generate-text";
pub const GENERATE_TEXT_REVISION: &str = "conduit.ai/generate-text@1";
pub const TEXT_VALUE_KIND: &str = "value/text@1";
pub const MAXIMUM_INPUT_BYTES: u64 = 262_144;
pub const MAXIMUM_CONTEXT_TOKENS: u64 = 262_144;
pub const MAXIMUM_OUTPUT_TOKENS: u64 = 16_384;
pub const MAXIMUM_TEMPERATURE_MILLI: u64 = 2_000;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenerateTextTerminalBehavior {
    OneOutputThenComplete,
    FailureWithoutOutput,
    CancelledWithoutSubstitution,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenerateTextFailure {
    InvalidUtf8,
    InputBoundExceeded,
    ContextBoundExceeded,
    OutputBoundExceeded,
    RealizationUnavailable,
    BaseFailure,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateTextContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub maximum_input_bytes: u64,
    pub maximum_context_tokens: u64,
    pub maximum_output_tokens: u64,
    pub terminal_behavior: [GenerateTextTerminalBehavior; 3],
    pub failures: [GenerateTextFailure; 7],
    pub limits: CapabilityLimits,
}

pub fn generate_text_contract() -> GenerateTextContract {
    GenerateTextContract {
        kind_id: kind_id(GENERATE_TEXT_KIND),
        kind_contract_revision: KindContractRevision::from(GENERATE_TEXT_REVISION),
        inputs: vec![text_port("prompt", PortDirection::Input)],
        outputs: vec![text_port("text", PortDirection::Output)],
        maximum_input_bytes: MAXIMUM_INPUT_BYTES,
        maximum_context_tokens: MAXIMUM_CONTEXT_TOKENS,
        maximum_output_tokens: MAXIMUM_OUTPUT_TOKENS,
        terminal_behavior: [
            GenerateTextTerminalBehavior::OneOutputThenComplete,
            GenerateTextTerminalBehavior::FailureWithoutOutput,
            GenerateTextTerminalBehavior::CancelledWithoutSubstitution,
        ],
        failures: [
            GenerateTextFailure::InvalidUtf8,
            GenerateTextFailure::InputBoundExceeded,
            GenerateTextFailure::ContextBoundExceeded,
            GenerateTextFailure::OutputBoundExceeded,
            GenerateTextFailure::RealizationUnavailable,
            GenerateTextFailure::BaseFailure,
            GenerateTextFailure::Cancelled,
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_INPUT_BYTES as u32,
        },
    }
}

fn text_port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(TEXT_VALUE_KIND),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_generate_text_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_form::{KindDefinition, KindSignature};

    startup.insert(KindSignature {
        kind: GENERATE_TEXT_KIND.to_string(),
        startup_parameters: vec![
            parameter("maximum-input-bytes", "Count", "4096"),
            parameter("maximum-context-tokens", "Count", "4096"),
            parameter("maximum-output-tokens", "Count", "512"),
            parameter("temperature-milli", "Count", "0"),
        ],
    })?;
    let contract = generate_text_contract();
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![
                count_field("maximum-input-bytes", 4096, 1, MAXIMUM_INPUT_BYTES),
                count_field("maximum-context-tokens", 4096, 1, MAXIMUM_CONTEXT_TOKENS),
                count_field("maximum-output-tokens", 512, 1, MAXIMUM_OUTPUT_TOKENS),
                count_field("temperature-milli", 0, 0, MAXIMUM_TEMPERATURE_MILLI),
            ],
        })
        .map_err(|error| error.to_string())
}

#[cfg(feature = "form-catalog")]
fn parameter(
    name: &str,
    value_type: &str,
    default: &str,
) -> conduit_form::StartupParameterSignature {
    use alloc::string::ToString;
    conduit_form::StartupParameterSignature {
        name: name.to_string(),
        value_type: value_type.to_string(),
        default: Some(default.to_string()),
    }
}

#[cfg(feature = "form-catalog")]
fn count_field(
    key: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> conduit_form::ConfigurationField {
    use alloc::string::ToString;
    conduit_form::ConfigurationField {
        key: key.to_string(),
        default_value: conduit_core::ConfigurationValue::U64(default),
        validation: conduit_form::ConfigurationRule::U64Range { minimum, maximum },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_is_typed_bounded_and_has_no_realization_facts() {
        let contract = generate_text_contract();
        assert_eq!(
            contract.inputs[0].value_kind,
            contract.outputs[0].value_kind
        );
        assert_eq!(contract.inputs[0].temporal, PortTemporal::Value);
        let encoded = serde_json::to_string(&contract).expect("contract serializes");
        for forbidden in ["OpenAI", "Anthropic", "CUDA", "Metal", "HTTP", "model_id"] {
            assert!(!encoded.contains(forbidden));
        }
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn authored_form_names_only_portable_generate_text_semantics() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_generate_text_catalog(&mut startup, &mut profile).expect("catalog installs");
        let source = "form answer {\n    generate: ai/generate-text\n}\n";
        let syntax = conduit_form::parse_syntax_document(source);
        let checked =
            conduit_form::check_syntax_document(&syntax, &startup).expect("portable syntax checks");
        let form = conduit_form::expand_canonical_form(&checked, "answer", &profile)
            .expect("portable form expands");
        assert_eq!(form.gears[0].kind_id.as_str(), GENERATE_TEXT_KIND);
        assert!(!source.contains("host"));
        assert!(!source.contains("base"));
        assert!(!source.contains("model"));
    }
}
