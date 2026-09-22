//! Ordinary form-facing contract for exact measurement summaries.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

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
        .insert(measurement_summary_kind_projection())
        .map_err(|error| error.to_string())
}

pub fn measurement_summary_kind_projection() -> KindProjection {
    KindProjection {
        kind_id: kind_id(MEASUREMENT_SUMMARY_KIND),
        kind_contract_revision: KindIdentity::from(MEASUREMENT_SUMMARY_CONTRACT_REVISION),
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

pub fn measurement_summary_semantic_contract() -> Kind {
    let definition = measurement_summary_kind_projection();
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
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
