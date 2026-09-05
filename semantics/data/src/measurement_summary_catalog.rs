//! Ordinary Form-facing contract for exact measurement summaries.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

use crate::{measurement_window_type, MEASUREMENT_SUMMARY_INFO_ID};

pub const MEASUREMENT_SUMMARY_KIND: &str = "data/measurement-summary";
pub const MEASUREMENT_SUMMARY_CONTRACT_REVISION: &str = "conduit.data/measurement-summary@1";

pub fn install_measurement_summary_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup
        .insert_structured_type("MeasurementSummary", measurement_summary_type())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: MEASUREMENT_SUMMARY_KIND.to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(measurement_summary_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn measurement_summary_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(MEASUREMENT_SUMMARY_KIND),
        kind_contract_revision: KindContractRevision::from(MEASUREMENT_SUMMARY_CONTRACT_REVISION),
        inputs: vec![port(
            "window",
            &measurement_window_type(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "summary",
            &measurement_summary_type(),
            PortDirection::Output,
        )],
        configuration: vec![],
    }
}

pub fn measurement_summary_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(MEASUREMENT_SUMMARY_INFO_ID))
        .expect("the measurement summary leaf identity is finite")
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
