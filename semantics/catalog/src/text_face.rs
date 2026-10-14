use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection};

pub const TEXT_FACE_KIND: &str = "face/text";
pub const TEXT_FACE_VALUE_KIND: &str = "value/text";
pub const TEXT_FACE_CONTRACT_REVISION: &str = "conduit.std/face-text@1";
/// Finite per-Play text occurrence budget. Eight admits the golden `hello`
/// interaction plus a small edit/refusal margin without making the live source
/// or Presenter unbounded.
pub const MAX_TEXT_VALUES: u64 = 8;

pub fn text_face_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TEXT_FACE_KIND),
        plain_name: "Text face".to_string(),
        summary: "Present up to four bounded UTF-8 text values on the host's text surface."
            .to_string(),
        inputs: text_face_inputs(),
        outputs: Vec::new(),
        configuration: {
            let mut fields = crate::data_label_configuration();
            fields.push(KindConfigurationField {
                key: "maximum-values".to_string(),
                default_value: conduit_core::ConfigurationValue::U64(MAX_TEXT_VALUES),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAX_TEXT_VALUES,
                },
            });
            fields
        },
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: conduit_text::MAX_TEXT_BYTES,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "show: face/text(\"Message\")".to_string(),
    }
}

pub fn text_face_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("text"),
        value_kind: kind_id(TEXT_FACE_VALUE_KIND),
        direction: PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    }]
}

#[cfg(feature = "plot-catalog")]
pub fn text_face_profile_catalog() -> conduit_plot::ProfileCatalog {
    use conduit_plot::ProfileCatalog;
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert_kind(text_face_contract().into_semantic_contract(TEXT_FACE_CONTRACT_REVISION))
        .expect("the one-kind text face catalog is unique");
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_text_face_contract_is_exact_and_hosted_only() {
        let contract = text_face_contract();
        assert_eq!(contract.inputs[0].value_kind.as_str(), TEXT_FACE_VALUE_KIND);
        assert!(contract.outputs.is_empty());
        assert!(contract.browser_manifestation_honest);
        assert!(!contract.pico_manifestation_honest);
    }
}
