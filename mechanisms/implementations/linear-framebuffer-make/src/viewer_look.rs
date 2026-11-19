//! Checked presentation intent for native Linux viewers. Absence preserves the ordinary Look.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue, ImplementationOffer,
    KindConfigurationField, KindConfigurationRule,
};

pub(super) const KIND: &str = "viewer/visual-preferences";
pub(super) const DRIVER: &str = "hosted/native-visual-preferences@1";

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: KIND.into(),
        implementation_id: DRIVER.into(),
        implementation_revision: 1,
        target_patterns: [
            "std/x86_64/computer",
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3",
            "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb",
        ]
        .map(String::from)
        .to_vec(),
        prerequisites: vec![],
        build_feature: None,
    }
}

pub(super) fn contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![KindConfigurationField {
            key: "look".into(),
            default_value: ConfigurationValue::Text("ordinary".into()),
            rule: KindConfigurationRule::TextOneOf {
                values: vec!["ordinary".into(), "picture-frame".into()],
            },
        }],
    }
}

pub(super) fn picture_frame(checked: &CheckedHostConfiguration) -> Result<bool, String> {
    let mut choices = checked
        .configuration()
        .bases
        .iter()
        .filter(|b| b.kind == KIND);
    let Some(base) = choices.next() else {
        return Ok(false);
    };
    if choices.next().is_some()
        || !checked
            .resolved_bases()
            .iter()
            .any(|(kind, driver)| kind == KIND && driver == DRIVER)
    {
        return Err("native viewer requires one checked visual preference".into());
    }
    let look = base
        .initialization
        .as_ref()
        .and_then(|init| init.values.get("look"))
        .and_then(|value| value.as_str());
    match look {
        Some("ordinary") => Ok(false),
        Some("picture-frame") => Ok(true),
        _ => Err("native viewer has no supported Look selection".into()),
    }
}
