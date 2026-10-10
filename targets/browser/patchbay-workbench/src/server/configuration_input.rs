//! Authored physical values enter through the same Rust catalogue checker as plots.
use conduit_core::ConfigurationValue;
use conduit_plot::{parse_checked_physical_value, CanonicalStartupValue, StartupCatalog};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ConfigurationInput {
    Checked(ConfigurationValue),
    Source(SourceConfigurationInput),
}
#[derive(Deserialize)]
pub(super) enum SourceConfigurationInput {
    #[serde(rename = "QuantitySource")]
    Quantity(String),
    #[serde(rename = "UnitSource")]
    Unit(String),
    #[serde(rename = "TemperatureDifferenceSource")]
    TemperatureDifference(String),
}
impl ConfigurationInput {
    #[cfg(test)]
    fn checked(self) -> Result<ConfigurationValue, String> {
        self.checked_with_catalog(&StartupCatalog::new())
    }
    pub(super) fn checked_with_catalog(
        self,
        catalog: &StartupCatalog,
    ) -> Result<ConfigurationValue, String> {
        let source = match self {
            Self::Checked(value) => return Ok(value),
            Self::Source(source) => source,
        };
        let spelling = match &source {
            SourceConfigurationInput::Quantity(source)
            | SourceConfigurationInput::Unit(source)
            | SourceConfigurationInput::TemperatureDifference(source) => source,
        };
        let checked = parse_checked_physical_value(spelling, None, catalog)
            .map_err(|error| format!("physical value: {error:?}"))?;
        match (source, checked) {
            (
                SourceConfigurationInput::Quantity(_),
                Some(CanonicalStartupValue::Quantity(value)),
            ) => Ok(ConfigurationValue::Quantity(value)),
            (SourceConfigurationInput::Unit(_), Some(CanonicalStartupValue::Unit(value))) => {
                Ok(ConfigurationValue::Unit(value))
            }
            (
                SourceConfigurationInput::TemperatureDifference(_),
                Some(CanonicalStartupValue::TemperatureDifference(value)),
            ) => Ok(ConfigurationValue::TemperatureDifference(value)),
            (
                SourceConfigurationInput::TemperatureDifference(_),
                Some(CanonicalStartupValue::Quantity(value)),
            ) => {
                let difference =
                    conduit_core::ExactTemperatureDifference::from_quantity(value.value())
                        .map_err(|error| format!("TemperatureDelta: {error:?}"))?;
                conduit_core::ExactTemperatureDifferenceConfigurationValue::new(
                    difference,
                    value.source().to_owned(),
                )
                .map(ConfigurationValue::TemperatureDifference)
                .ok_or_else(|| "TemperatureDelta evidence exceeds admission bound".into())
            }
            _ => Err("physical source does not have the requested role".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_unit_source_uses_the_checked_document_snapshot() {
        let source = conduit_plot::parse_syntax_document(
            "unit smoot : Distance = { reference: m, scale: 1.7018 }",
        );
        let catalog =
            conduit_plot::checked_physical_catalog_for_document(&source, &StartupCatalog::new())
                .unwrap();
        let input: ConfigurationInput =
            serde_json::from_str(r#"{"QuantitySource":"1smoot"}"#).unwrap();
        let ConfigurationValue::Quantity(value) = input.checked_with_catalog(&catalog).unwrap()
        else {
            panic!("Quantity")
        };
        let realized = value.value().convert(conduit_core::Unit::Meter).unwrap();
        assert_eq!(realized.coefficient(), 17018);
        assert_eq!(realized.exponent(), -4);
        let point: ConfigurationInput =
            serde_json::from_str(r#"{"TemperatureDifferenceSource":"9°F"}"#).unwrap();
        assert!(point.checked_with_catalog(&catalog).is_err());
        let delta: ConfigurationInput =
            serde_json::from_str(r#"{"TemperatureDifferenceSource":"TemperatureDelta(9, °F)"}"#)
                .unwrap();
        assert!(matches!(
            delta.checked_with_catalog(&catalog).unwrap(),
            ConfigurationValue::TemperatureDifference(_)
        ));
    }
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
