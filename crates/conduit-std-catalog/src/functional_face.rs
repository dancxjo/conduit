use crate::StandardConfigurationField;
use alloc::string::ToString;
use alloc::vec::Vec;
use conduit_core::{ConfigurationValue, FaceStartupParameter};

pub(crate) fn startup_face(fields: &[StandardConfigurationField]) -> Vec<FaceStartupParameter> {
    fields
        .iter()
        .map(|field| FaceStartupParameter {
            name: field.key.clone(),
            value_type: match field.default_value {
                ConfigurationValue::Bool(_) => "Boolean",
                ConfigurationValue::U64(_) => "Count",
            }
            .to_string(),
            has_default: true,
        })
        .collect()
}
