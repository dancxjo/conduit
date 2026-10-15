//! A finite, visibly presented choice whose identity is retained before emission.
use crate::{KindTerminalBehavior, StandardKindContract};
#[cfg(feature = "plot-catalog")]
use alloc::string::String;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, Kind, KindConfigurationField,
    KindConfigurationRule, KindIdentity, KindSemanticLaw, PortDescriptor, PortDirection,
    PortTemporal, StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue,
};

pub const SAVED_CHOICE_KIND: &str = "input/saved-choice";
pub const SAVED_CHOICE_REVISION: &str = "conduit.input/saved-choice@1";
pub const SAVED_CHOICE_MAXIMUM_OPTIONS: u16 = 16;
pub const SAVED_CHOICE_MAXIMUM_ID_BYTES: u32 = 64;
pub const SAVED_CHOICE_MAXIMUM_LABEL_BYTES: u32 = 128;
pub const SAVED_CHOICE_MAXIMUM_KEY_BYTES: u32 = 128;

pub fn saved_choice_option_type() -> StructuredInfoType {
    let text = StructuredInfoType::leaf(kind_id("value/text")).expect("text type");
    StructuredInfoType::record(
        kind_id("conduit.input/saved-choice-option@1"),
        vec![
            StructuredFieldType::new("identity", text.clone()).expect("identity field"),
            StructuredFieldType::new("label", text).expect("label field"),
        ],
    )
    .expect("finite choice option")
}
pub fn saved_choice_options_type() -> StructuredInfoType {
    StructuredInfoType::bounded_sequence(
        saved_choice_option_type(),
        1,
        SAVED_CHOICE_MAXIMUM_OPTIONS,
    )
    .expect("finite options")
}
fn default_options() -> StructuredConfigurationValue {
    let text = |value: &str| {
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/text")).expect("text type"),
            value.as_bytes().to_vec(),
        )
        .expect("text value")
    };
    let option = StructuredInfoValue::record(
        saved_choice_option_type(),
        vec![
            StructuredFieldValue::new("identity", text("default")).expect("identity field"),
            StructuredFieldValue::new("label", text("Default")).expect("label field"),
        ],
    )
    .expect("default option");
    let value = StructuredInfoValue::sequence(saved_choice_options_type(), vec![option])
        .expect("default options");
    StructuredConfigurationValue::new(
        saved_choice_options_type()
            .profile()
            .expect("options profile")
            .value_kind()
            .clone(),
        value.canonical_bytes().expect("canonical options"),
    )
    .expect("options configuration")
}
fn text_field(key: &str, value: &str, maximum: u32) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(value.into()),
        rule: KindConfigurationRule::TextBytes { maximum },
    }
}
pub fn saved_choice_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(SAVED_CHOICE_KIND),
        plain_name: "Saved choice".into(),
        summary: "Present a finite named choice, restore its retained identity, and save each choice before emitting it.".into(),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("chosen"),
            value_kind: kind_id("value/text"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
            abnormal_kind: None,
        }],
        configuration: vec![
            text_field("label", "Choice", SAVED_CHOICE_MAXIMUM_LABEL_BYTES),
            text_field("key", "choice", SAVED_CHOICE_MAXIMUM_KEY_BYTES),
            text_field("initial", "default", SAVED_CHOICE_MAXIMUM_ID_BYTES),
            KindConfigurationField {
                key: "options".into(),
                default_value: ConfigurationValue::Structured(default_options()),
                rule: KindConfigurationRule::Structured {
                    profile: saved_choice_options_type().profile().expect("options profile").value_kind().clone(),
                },
            },
        ],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: SAVED_CHOICE_MAXIMUM_ID_BYTES,
        },
        terminal_behavior: KindTerminalBehavior::HostInputEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "language: input/saved-choice(label = \"Language\", key = \"language\", initial = \"default\")".into(),
    }
}
pub fn saved_choice_semantic_contract() -> Kind {
    let contract = saved_choice_contract();
    Kind {
        startup_parameters: crate::startup_front(&contract.configuration),
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: KindIdentity::from(SAVED_CHOICE_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
        semantic_laws: vec![KindSemanticLaw::Terminal(contract.terminal_behavior)],
        limits: contract.limits,
    }
}
#[cfg(feature = "plot-catalog")]
pub fn install_saved_choice_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    use alloc::string::ToString;
    let kind = saved_choice_semantic_contract();
    startup.insert_structured_type("SavedChoiceOptions", saved_choice_options_type())?;
    let startup_parameters = vec![
        conduit_plot::StartupParameterSignature {
            name: "label".into(),
            value_type: "Text".into(),
            default: Some("\"Choice\"".into()),
        },
        conduit_plot::StartupParameterSignature {
            name: "key".into(),
            value_type: "Text".into(),
            default: Some("\"choice\"".into()),
        },
        conduit_plot::StartupParameterSignature {
            name: "initial".into(),
            value_type: "Text".into(),
            default: Some("\"default\"".into()),
        },
        conduit_plot::StartupParameterSignature {
            name: "options".into(),
            value_type: "SavedChoiceOptions".into(),
            default: Some("[{identity: \"default\", label: \"Default\"}]".into()),
        },
    ];
    startup.insert(conduit_plot::KindSignature {
        kind: SAVED_CHOICE_KIND.into(),
        startup_parameters,
    })?;
    profile.insert_kind(kind).map_err(|error| error.to_string())
}

#[cfg(all(test, feature = "plot-catalog"))]
mod tests {
    use super::*;
    #[test]
    fn finite_named_choices_are_authored_structured_values_with_text_identity_output() {
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        install_saved_choice_catalogs(&mut startup, &mut profile).unwrap();
        let source = r#"plot preferences {
            traditions = [{ identity: "first", label: "First tradition" }, { identity: "second", label: "Second tradition" }]
            choice: input/saved-choice(label = "Tradition", key = "tradition", initial = "first", options = traditions)
        }"#;
        let checked = conduit_plot::parse_with_startup(source, &startup, &profile).unwrap();
        assert_eq!(checked.gears[0].outputs[0].port_id.as_str(), "chosen");
        assert_eq!(
            checked.gears[0].outputs[0].value_kind.as_str(),
            "value/text"
        );
        assert!(checked.gears[0]
            .configuration
            .iter()
            .any(|entry| entry.key == "options"
                && matches!(entry.value, ConfigurationValue::Structured(_))));
        assert_eq!(saved_choice_contract().limits.max_queue_items, 1);
        let bad = source.replace("options = traditions", "options = \"first\"");
        assert!(conduit_plot::parse_with_startup(&bad, &startup, &profile).is_err());
    }
}
