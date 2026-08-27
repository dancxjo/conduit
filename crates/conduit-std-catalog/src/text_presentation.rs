use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
};

pub const TEXT_PRESENTATION_KIND: &str = "presentation/text";
pub const TEXT_PRESENTATION_VALUE_KIND: &str = "value/text@1";
pub const TEXT_PRESENTATION_CONTRACT_REVISION: &str = "conduit.std/presentation-text@1";
/// Finite per-Play text occurrence budget. Eight admits the golden `hello`
/// interaction plus a small edit/refusal margin without making the live source
/// or Presenter unbounded.
pub const MAX_TEXT_VALUES: u64 = 8;

pub fn text_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TEXT_PRESENTATION_KIND),
        plain_name: "Text presentation".to_string(),
        summary: "Present up to four bounded UTF-8 text values on the host's text surface."
            .to_string(),
        inputs: text_presentation_inputs(),
        outputs: Vec::new(),
        configuration: vec![StandardConfigurationField {
            key: "maximum-values".to_string(),
            default_value: conduit_core::ConfigurationValue::U64(MAX_TEXT_VALUES),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: MAX_TEXT_VALUES,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: conduit_text::MAX_TEXT_BYTES,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "show: presentation/text".to_string(),
    }
}

pub fn text_presentation_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("text"),
        value_kind: kind_id(TEXT_PRESENTATION_VALUE_KIND),
        direction: PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

#[cfg(feature = "form-catalog")]
pub fn text_presentation_profile_catalog() -> conduit_form::ProfileCatalog {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(TEXT_PRESENTATION_KIND),
            kind_contract_revision: KindContractRevision::from(TEXT_PRESENTATION_CONTRACT_REVISION),
            inputs: text_presentation_inputs(),
            outputs: Vec::new(),
            configuration: vec![ConfigurationField {
                key: "maximum-values".to_string(),
                default_value: conduit_core::ConfigurationValue::U64(MAX_TEXT_VALUES),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAX_TEXT_VALUES,
                },
            }],
        })
        .expect("the one-kind text presentation catalog is unique");
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_text_presentation_contract_is_exact_and_hosted_only() {
        let contract = text_presentation_contract();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            TEXT_PRESENTATION_VALUE_KIND
        );
        assert!(contract.outputs.is_empty());
        assert!(contract.browser_manifestation_honest);
        assert!(!contract.pico_manifestation_honest);
    }
}
