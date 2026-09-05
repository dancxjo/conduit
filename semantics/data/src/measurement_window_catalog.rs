//! Ordinary Form-facing contract for a finite measurement window.

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
    MAXIMUM_MEASUREMENT_WINDOW_SAMPLES, MEASUREMENT_SAMPLE_INFO_ID, MEASUREMENT_WINDOW_INFO_ID,
};

pub const MEASUREMENT_COUNT_WINDOW_KIND: &str = "data/measurement-count-window";
pub const MEASUREMENT_WINDOW_CONTRACT_REVISION: &str = "conduit.data/measurement-window@1";

pub fn install_measurement_window_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup
        .insert_structured_type("MeasurementSample", measurement_sample_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type("MeasurementWindow", measurement_window_type())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: MEASUREMENT_COUNT_WINDOW_KIND.to_string(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "count".to_string(),
                value_type: "Count".to_string(),
                default: Some("8".to_string()),
            },
            StartupParameterSignature {
                name: "when-full".to_string(),
                value_type: "Text".to_string(),
                default: Some("reject".to_string()),
            },
        ],
    })?;
    profile
        .insert(measurement_window_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn measurement_window_kind_definition() -> KindDefinition {
    let sample = measurement_sample_type();
    let window = measurement_window_type();
    KindDefinition {
        kind_id: kind_id(MEASUREMENT_COUNT_WINDOW_KIND),
        kind_contract_revision: KindContractRevision::from(MEASUREMENT_WINDOW_CONTRACT_REVISION),
        inputs: vec![port(
            "measurement",
            &sample,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "window",
            &window,
            PortDirection::Output,
            PortTemporal::Value,
        )],
        configuration: vec![
            ConfigurationField {
                key: "count".to_string(),
                default_value: ConfigurationValue::U64(8),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAXIMUM_MEASUREMENT_WINDOW_SAMPLES as u64,
                },
            },
            ConfigurationField {
                key: "when-full".to_string(),
                default_value: ConfigurationValue::Text("reject".to_string()),
                validation: ConfigurationRule::TextOneOf {
                    values: vec!["reject".to_string(), "drop-oldest".to_string()],
                },
            },
        ],
    }
}

pub fn measurement_sample_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(MEASUREMENT_SAMPLE_INFO_ID))
        .expect("the measurement sample leaf identity is finite")
}

pub fn measurement_window_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(MEASUREMENT_WINDOW_INFO_ID))
        .expect("the measurement window leaf identity is finite")
}

fn port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal,
    }
}
