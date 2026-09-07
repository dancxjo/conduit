//! Ordinary Form-facing contract for finite measurement plot projection.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

use crate::{
    measurement_window_type, MAXIMUM_MEASUREMENT_PLOT_POINTS, MEASUREMENT_PLOT_SERIES_INFO_ID,
};

pub const MEASUREMENT_PLOT_KIND: &str = "data/measurement-plot";
pub const MEASUREMENT_PLOT_CONTRACT_REVISION: &str = "conduit.data/measurement-plot@1";

pub fn install_measurement_plot_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup
        .insert_structured_type("MeasurementPlotSeries", measurement_plot_series_type())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: MEASUREMENT_PLOT_KIND.to_string(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "points".to_string(),
                value_type: "Count".to_string(),
                default: Some("8".to_string()),
            },
            StartupParameterSignature {
                name: "when-full".to_string(),
                value_type: "Text".to_string(),
                default: Some("evenly-spaced".to_string()),
            },
        ],
    })?;
    profile
        .insert(measurement_plot_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn measurement_plot_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(MEASUREMENT_PLOT_KIND),
        kind_contract_revision: KindContractRevision::from(MEASUREMENT_PLOT_CONTRACT_REVISION),
        inputs: vec![port(
            "window",
            &measurement_window_type(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "series",
            &measurement_plot_series_type(),
            PortDirection::Output,
        )],
        configuration: vec![
            ConfigurationField {
                key: "points".to_string(),
                default_value: ConfigurationValue::U64(8),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAXIMUM_MEASUREMENT_PLOT_POINTS as u64,
                },
            },
            ConfigurationField {
                key: "when-full".to_string(),
                default_value: ConfigurationValue::Text("evenly-spaced".to_string()),
                validation: ConfigurationRule::TextOneOf {
                    values: vec!["reject".to_string(), "evenly-spaced".to_string()],
                },
            },
        ],
    }
}

pub fn measurement_plot_series_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(MEASUREMENT_PLOT_SERIES_INFO_ID))
        .expect("the measurement plot series leaf identity is finite")
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
