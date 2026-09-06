//! Count both Boolean outcomes in one finite collection of records.

use crate::{JsonRefusal, JsonValue, PortableKindContract};
use alloc::{string::ToString, vec};
use conduit_core::{kind_id, KindContractRevision, Scalar};

pub const JSON_BOOLEAN_SUMMARY_KIND: &str = "json/boolean-summary";
pub const JSON_BOOLEAN_SUMMARY_REVISION: &str = "conduit.json/boolean-summary@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum JsonSummaryRefusal {
    InvalidField,
    NotCollection,
    NotRecord,
    MissingField,
    NotBoolean,
    InvalidValue(JsonRefusal),
}

impl JsonSummaryRefusal {
    pub const fn detail(self) -> u16 {
        match self {
            Self::InvalidField => 120,
            Self::NotCollection => 121,
            Self::NotRecord => 122,
            Self::MissingField => 123,
            Self::NotBoolean => 124,
            Self::InvalidValue(error) => error as u16,
        }
    }
}

pub fn json_boolean_summary(
    collection: &JsonValue,
    field: &str,
) -> Result<JsonValue, JsonSummaryRefusal> {
    if field.is_empty() || field.len() > crate::JSON_MAXIMUM_KEY_BYTES {
        return Err(JsonSummaryRefusal::InvalidField);
    }
    collection
        .validate()
        .map_err(JsonSummaryRefusal::InvalidValue)?;
    let JsonValue::Array(records) = collection else {
        return Err(JsonSummaryRefusal::NotCollection);
    };
    let mut true_count = 0;
    for record in records {
        let JsonValue::Object(fields) = record else {
            return Err(JsonSummaryRefusal::NotRecord);
        };
        let value = fields
            .iter()
            .find(|(key, _)| key == field)
            .map(|(_, value)| value)
            .ok_or(JsonSummaryRefusal::MissingField)?;
        match value {
            JsonValue::Bool(true) => true_count += 1,
            JsonValue::Bool(false) => {}
            _ => return Err(JsonSummaryRefusal::NotBoolean),
        }
    }
    let count =
        |value: usize| JsonValue::Number(Scalar::from_raw_microunits(value as i64 * 1_000_000));
    Ok(JsonValue::Object(vec![
        ("false".to_string(), count(records.len() - true_count)),
        ("total".to_string(), count(records.len())),
        ("true".to_string(), count(true_count)),
    ]))
}

pub fn json_boolean_summary_semantics() -> PortableKindContract {
    let mut contract = crate::json_collection_step_semantics();
    contract.kind_id = kind_id(JSON_BOOLEAN_SUMMARY_KIND);
    contract.kind_contract_revision = KindContractRevision::from(JSON_BOOLEAN_SUMMARY_REVISION);
    contract
}

#[cfg(feature = "form-catalog")]
pub fn install_json_boolean_summary_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_core::ConfigurationValue;
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    let contract = json_boolean_summary_semantics();
    startup.insert(KindSignature {
        kind: JSON_BOOLEAN_SUMMARY_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "field".into(),
            value_type: "Text".into(),
            default: Some("\"enabled\"".into()),
        }],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![ConfigurationField {
                key: "field".into(),
                default_value: ConfigurationValue::Text("enabled".into()),
                validation: ConfigurationRule::TextBytes {
                    maximum: crate::JSON_MAXIMUM_KEY_BYTES as u32,
                },
            }],
        })
        .map_err(|error| error.to_string())
}
