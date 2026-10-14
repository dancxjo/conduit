//! Explicit finite names supplied when constructing authored Face Gears.
use crate::{
    ConfigurationEntry, ConfigurationValue, FrontStartupParameter, KindConfigurationField,
    KindConfigurationRule,
};
use alloc::{string::String, vec, vec::Vec};

pub const MAXIMUM_DATA_LABEL_BYTES: u32 = 128;

pub fn valid_data_label(value: &str) -> bool {
    value.len() <= MAXIMUM_DATA_LABEL_BYTES as usize
        && !value.trim().is_empty()
        && !value.chars().any(char::is_control)
}

pub fn data_label_configuration() -> Vec<KindConfigurationField> {
    vec![KindConfigurationField {
        key: "label".into(),
        default_value: ConfigurationValue::Text(String::new()),
        rule: KindConfigurationRule::DataLabel {
            maximum: MAXIMUM_DATA_LABEL_BYTES,
        },
    }]
}

pub fn data_label_startup_front() -> Vec<FrontStartupParameter> {
    vec![FrontStartupParameter {
        name: "label".into(),
        value_type: crate::kind_id(crate::TEXT_INFO_ID),
        has_default: false,
    }]
}

pub fn face_data_label(configuration: &[ConfigurationEntry]) -> Result<&str, String> {
    let mut labels = configuration.iter().filter(|entry| entry.key == "label");
    let Some(entry) = labels.next() else {
        return Err("Face requires a construction label".into());
    };
    let ConfigurationValue::Text(value) = &entry.value else {
        return Err("Face label must be Text".into());
    };
    if labels.next().is_some() || !valid_data_label(value) {
        return Err("Face label must be unique, nonblank, control-free bounded Text".into());
    }
    Ok(value)
}
