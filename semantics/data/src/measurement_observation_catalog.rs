//! Portable conversion of one exact quantity into one typed observation.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, QUANTITY_INFO_ID,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

use crate::measurement_sample_type;

pub const MEASUREMENT_OBSERVATION_KIND: &str = "data/measurement-observation";
pub const MEASUREMENT_OBSERVATION_REVISION: &str = "conduit.data/measurement-observation@1";
pub const MAXIMUM_MEASUREMENT_CLOCK_BASIS_BYTES: u32 = 64;

pub fn install_measurement_observation_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(KindSignature {
        kind: MEASUREMENT_OBSERVATION_KIND.to_string(),
        startup_parameters: vec![parameter("clock-basis", "Text", "\"control-occurrence\"")],
    })?;
    profile
        .insert(measurement_observation_definition())
        .map_err(|error| error.to_string())
}

pub fn measurement_observation_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(MEASUREMENT_OBSERVATION_KIND),
        kind_contract_revision: KindContractRevision::from(MEASUREMENT_OBSERVATION_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("quantity"),
            value_kind: kind_id(QUANTITY_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("measurement"),
            value_kind: measurement_sample_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: vec![ConfigurationField {
            key: "clock-basis".into(),
            default_value: ConfigurationValue::Text("control-occurrence".into()),
            validation: ConfigurationRule::TextBytes {
                maximum: MAXIMUM_MEASUREMENT_CLOCK_BASIS_BYTES,
            },
        }],
    }
}

fn parameter(name: &str, value_type: &str, default: &str) -> StartupParameterSignature {
    StartupParameterSignature {
        name: name.into(),
        value_type: value_type.into(),
        default: Some(default.into()),
    }
}
