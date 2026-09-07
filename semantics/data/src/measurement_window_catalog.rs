//! Ordinary Form-facing contract for a finite measurement window.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

use crate::{
    MEASUREMENT_SAMPLE_INFO_ID, MEASUREMENT_WINDOW_INFO_ID, MEASUREMENT_WINDOW_PROFILE_INFO_ID,
};

pub const MEASUREMENT_COUNT_WINDOW_KIND: &str = "data/measurement-count-window";
pub const MEASUREMENT_WINDOW_CONTRACT_REVISION: &str = "conduit.data/measurement-window@2";

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
    startup
        .insert_structured_type(
            "MeasurementWindowProfile",
            measurement_window_profile_type(),
        )
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: MEASUREMENT_COUNT_WINDOW_KIND.to_string(),
        startup_parameters: vec![],
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
        inputs: vec![
            port(
                "profile",
                &measurement_window_profile_type(),
                PortDirection::Input,
                PortTemporal::Value,
            ),
            port(
                "measurement",
                &sample,
                PortDirection::Input,
                PortTemporal::Flow { closes: true },
            ),
        ],
        outputs: vec![port(
            "window",
            &window,
            PortDirection::Output,
            PortTemporal::Value,
        )],
        configuration: vec![],
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

pub fn measurement_window_profile_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(MEASUREMENT_WINDOW_PROFILE_INFO_ID))
        .expect("the measurement window profile leaf identity is finite")
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
