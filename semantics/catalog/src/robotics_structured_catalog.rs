//! Portable structured-robotics Form catalog.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, KindId, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    robotics_contact_event_type, robotics_motion_request_type, robotics_pose_sample_type,
    robotics_power_telemetry_type, robotics_range_observation_type,
    robotics_structured_registered_types,
};

pub const DETERMINISTIC_ROBOTICS_OBSERVATIONS_KIND: &str = "robotics/deterministic-observations";
pub const DETERMINISTIC_ROBOTICS_INTENT_KIND: &str = "robotics/deterministic-intent";
pub const ROBOTICS_EXECUTE_MOTION_KIND: &str = "robotics/execute-motion";
pub const ROBOTICS_STRUCTURED_REVISION: &str = "conduit.std/robotics-structured@1";

pub type RoboticsStructuredKindContract = (KindId, Vec<PortDescriptor>, Vec<PortDescriptor>);

/// Exact portable structured-robotics Kinds and typed faces.
pub fn robotics_structured_kind_contracts() -> Vec<RoboticsStructuredKindContract> {
    vec![
        (
            kind_id(DETERMINISTIC_ROBOTICS_OBSERVATIONS_KIND),
            vec![],
            observation_outputs(),
        ),
        (
            kind_id(DETERMINISTIC_ROBOTICS_INTENT_KIND),
            vec![],
            vec![port(
                "request",
                &robotics_motion_request_type(),
                PortDirection::Output,
            )],
        ),
        (
            kind_id(ROBOTICS_EXECUTE_MOTION_KIND),
            vec![port(
                "request",
                &robotics_motion_request_type(),
                PortDirection::Input,
            )],
            vec![],
        ),
    ]
}

pub fn install_robotics_structured_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in robotics_structured_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for (kind, inputs, outputs) in robotics_structured_kind_contracts() {
        insert_kind(startup, profile, kind.as_str(), inputs, outputs)?;
    }
    Ok(())
}

fn observation_outputs() -> Vec<PortDescriptor> {
    vec![
        port(
            "contact",
            &robotics_contact_event_type(),
            PortDirection::Output,
        ),
        port("pose", &robotics_pose_sample_type(), PortDirection::Output),
        port(
            "power",
            &robotics_power_telemetry_type(),
            PortDirection::Output,
        ),
        port(
            "range",
            &robotics_range_observation_type(),
            PortDirection::Output,
        ),
    ]
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(ROBOTICS_STRUCTURED_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed robotics structured profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
