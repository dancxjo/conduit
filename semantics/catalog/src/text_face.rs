use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection};

pub const TEXT_FACE_KIND: &str = "face/text";
pub const TEXT_FACE_VALUE_KIND: &str = "value/text";
pub const TEXT_FACE_CONTRACT_REVISION: &str = "conduit.std/face-text@2";
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
            fields.push(KindConfigurationField {
                key: "insertion-caret".to_string(),
                default_value: conduit_core::ConfigurationValue::Bool(false),
                rule: KindConfigurationRule::Any,
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

/// Semantic end insertion point; presentation style belongs to the selected Look.
/// Planned configuration has already been checked against the Boolean parameter.
pub fn text_face_insertion_caret(configuration: &[conduit_core::ConfigurationEntry]) -> bool {
    configuration.iter().any(|entry| {
        entry.key == "insertion-caret"
            && entry.value == conduit_core::ConfigurationValue::Bool(true)
    })
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
    fn insertion_caret_is_an_explicit_boolean_end_position_cue() {
        let contract = text_face_contract();
        let caret = contract
            .configuration
            .iter()
            .find(|field| field.key == "insertion-caret")
            .unwrap();
        assert_eq!(
            caret.default_value,
            conduit_core::ConfigurationValue::Bool(false)
        );
        assert!(!text_face_insertion_caret(&[]));
        for value in [false, true] {
            assert_eq!(
                text_face_insertion_caret(&[conduit_core::ConfigurationEntry {
                    key: "insertion-caret".into(),
                    value: conduit_core::ConfigurationValue::Bool(value)
                }]),
                value
            );
        }
    }

    #[cfg(feature = "plot-catalog")]
    #[test]
    fn installed_text_face_startup_fore_matches_the_complete_semantic_contract() {
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        crate::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
        let signature = startup.signature(TEXT_FACE_KIND).unwrap();
        let actual = startup.canonical_startup_parameters(signature).unwrap();
        let contract = text_face_contract().into_semantic_contract(TEXT_FACE_CONTRACT_REVISION);
        assert_eq!(actual, contract.startup_parameters);
        for (configuration, expected) in [
            ("", false),
            (", insertion-caret = false", false),
            (", insertion-caret = true", true),
        ] {
            let source = alloc::format!("plot cue {{\n text: text/literal(\"hello\")\n show: face/text(\"Message\"{configuration})\n text >> show.text\n}}\n");
            let checked = conduit_plot::check_syntax_document(
                &conduit_plot::parse_syntax_document(&source),
                &startup,
            )
            .unwrap();
            let expanded =
                conduit_plot::expand_canonical_plot_for_authoring(&checked, "cue", &profile)
                    .unwrap()
                    .expanded;
            let face = expanded
                .gears
                .iter()
                .find(|gear| gear.kind_id.as_str() == TEXT_FACE_KIND)
                .unwrap();
            assert_eq!(text_face_insertion_caret(&face.configuration), expected);
        }
    }

    #[test]
    fn typed_text_face_contract_is_exact_and_hosted_only() {
        let contract = text_face_contract();
        assert_eq!(contract.inputs[0].value_kind.as_str(), TEXT_FACE_VALUE_KIND);
        assert!(contract.outputs.is_empty());
        assert!(contract.browser_manifestation_honest);
        assert!(!contract.pico_manifestation_honest);
    }
}
