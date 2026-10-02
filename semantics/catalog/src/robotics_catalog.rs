use super::{
    robotics_contracts_with_revisions, robotics_hazard_contracts_with_revisions,
    robotics_input_contracts_with_revisions, KindConfigurationField, KindConfigurationRule,
};
use alloc::format;
use alloc::string::{String, ToString};
use conduit_core::ConfigurationValue;
use conduit_plot::{KindSignature, StartupParameterSignature};

pub fn install_robotics_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (contract, revision) in robotics_contracts_with_revisions()
        .into_iter()
        .chain(robotics_hazard_contracts_with_revisions())
        .chain(robotics_input_contracts_with_revisions())
    {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: configuration_type(field).to_string(),
                    default: Some(configuration_source(field)),
                })
                .collect(),
        })?;
        profile
            .insert_kind(contract.into_semantic_contract(revision))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn configuration_type(field: &KindConfigurationField) -> &'static str {
    match (&field.rule, &field.default_value) {
        (
            KindConfigurationRule::QuantityRange { canonical_unit, .. },
            ConfigurationValue::Quantity(_),
        ) => match canonical_unit.dimension() {
            conduit_core::QuantityDimension::Time => "Duration",
            conduit_core::QuantityDimension::Frequency => "Frequency",
            conduit_core::QuantityDimension::Voltage => "Voltage",
            conduit_core::QuantityDimension::Temperature => "Temperature",
            conduit_core::QuantityDimension::Length => "Distance",
            conduit_core::QuantityDimension::Angle => "Angle",
            conduit_core::QuantityDimension::Ratio => "Ratio",
            conduit_core::QuantityDimension::PixelCount => "PixelCount",
            conduit_core::QuantityDimension::Current
            | conduit_core::QuantityDimension::Charge
            | conduit_core::QuantityDimension::DataSize
            | conduit_core::QuantityDimension::Mass
            | conduit_core::QuantityDimension::Area
            | conduit_core::QuantityDimension::Volume
            | conduit_core::QuantityDimension::Speed
            | conduit_core::QuantityDimension::Acceleration
            | conduit_core::QuantityDimension::Force
            | conduit_core::QuantityDimension::Energy
            | conduit_core::QuantityDimension::Power
            | conduit_core::QuantityDimension::Pressure => "Quantity",
        },
        (_, ConfigurationValue::Text(_)) => "Text",
        (_, ConfigurationValue::U64(_)) => "Count",
        (_, ConfigurationValue::I64(_)) => "Scalar",
        (_, ConfigurationValue::Quantity(_)) => "Quantity",
        _ => unreachable!("robotics configuration is finite text/integer/quantity"),
    }
}

fn configuration_source(field: &KindConfigurationField) -> String {
    match &field.default_value {
        ConfigurationValue::Text(value) => format!("\"{value}\""),
        ConfigurationValue::U64(value) => value.to_string(),
        ConfigurationValue::I64(value) => value.to_string(),
        ConfigurationValue::Quantity(value) => {
            format!("{}{}", value.value(), value.unit().plot_suffix())
        }
        _ => unreachable!("robotics configuration is finite text/integer"),
    }
}
