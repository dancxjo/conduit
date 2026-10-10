//! Authored physical values enter through the same Rust catalogue checker as plots.
use conduit_core::{
    ConfigurationValue, ExactTemperatureDifferenceConfigurationValue, QuantityConfigurationValue,
    UnitConfigurationValue,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ConfigurationInput {
    Checked(ConfigurationValue),
    Source(SourceConfigurationInput),
}
#[derive(Deserialize)]
pub(super) enum SourceConfigurationInput {
    QuantitySource(String),
    UnitSource(String),
    TemperatureDifferenceSource(String),
}
impl ConfigurationInput {
    pub(super) fn checked(self) -> Result<ConfigurationValue, String> {
        match self {
            Self::Checked(value) => Ok(value),
            Self::Source(SourceConfigurationInput::QuantitySource(source)) => {
                QuantityConfigurationValue::parse(&source)
                    .map(ConfigurationValue::Quantity)
                    .map_err(|error| format!("Quantity: {error:?}"))
            }
            Self::Source(SourceConfigurationInput::UnitSource(source)) => {
                UnitConfigurationValue::parse(&source)
                    .map(ConfigurationValue::Unit)
                    .map_err(|error| format!("Unit: {error:?}"))
            }
            Self::Source(SourceConfigurationInput::TemperatureDifferenceSource(source)) => {
                ExactTemperatureDifferenceConfigurationValue::parse(&source)
                    .map(ConfigurationValue::TemperatureDifference)
                    .map_err(|error| format!("TemperatureDifference: {error:?}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_source_is_checked_in_rust_and_keeps_evidence() {
        let input: ConfigurationInput =
            serde_json::from_str(r#"{"QuantitySource":"1Qm"}"#).unwrap();
        let ConfigurationValue::Quantity(value) = input.checked().unwrap() else {
            panic!("Quantity")
        };
        assert_eq!(value.source(), "1Qm");
        assert_eq!(
            value.value().unit(),
            conduit_core::Unit::resolve("Qm").unwrap()
        );
        for source in [
            r#"{"QuantitySource":"21C"}"#,
            r#"{"UnitSource":"unknown"}"#,
            r#"{"TemperatureDifferenceSource":"1Hz"}"#,
        ] {
            let input: ConfigurationInput = serde_json::from_str(source).unwrap();
            assert!(input.checked().is_err());
        }
    }
}
