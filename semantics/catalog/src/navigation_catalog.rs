//! Form catalog for the finite portable navigation waist.

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
    navigation_control_type, navigation_goal_type, navigation_pose_type,
    navigation_registered_types, navigation_route_decision_type, navigation_route_type,
    navigation_time_type, navigation_trajectory_type, navigation_traversability_type,
};

pub const NAVIGATION_ROUTE_GRID4_KIND: &str = "navigation/route-grid4";
pub const NAVIGATION_TIME_PARAMETERIZE_KIND: &str = "navigation/time-parameterize";
pub const NAVIGATION_LOCAL_CONTROL_KIND: &str = "navigation/local-control";
pub const NAVIGATION_REVISION: &str = "conduit.navigation/portable-navigation@1";

pub type NavigationKindContract = (KindId, Vec<PortDescriptor>, Vec<PortDescriptor>);

pub fn navigation_kind_contracts() -> Vec<NavigationKindContract> {
    vec![
        (
            kind_id(NAVIGATION_ROUTE_GRID4_KIND),
            vec![
                port("pose", &navigation_pose_type(), PortDirection::Input),
                port("goal", &navigation_goal_type(), PortDirection::Input),
                port(
                    "traversability",
                    &navigation_traversability_type(),
                    PortDirection::Input,
                ),
                port("time", &navigation_time_type(), PortDirection::Input),
            ],
            vec![port(
                "decision",
                &navigation_route_decision_type(),
                PortDirection::Output,
            )],
        ),
        (
            kind_id(NAVIGATION_TIME_PARAMETERIZE_KIND),
            vec![
                port("route", &navigation_route_type(), PortDirection::Input),
                port("time", &navigation_time_type(), PortDirection::Input),
            ],
            vec![port(
                "trajectory",
                &navigation_trajectory_type(),
                PortDirection::Output,
            )],
        ),
        (
            kind_id(NAVIGATION_LOCAL_CONTROL_KIND),
            vec![
                port("pose", &navigation_pose_type(), PortDirection::Input),
                port(
                    "trajectory",
                    &navigation_trajectory_type(),
                    PortDirection::Input,
                ),
                port("time", &navigation_time_type(), PortDirection::Input),
            ],
            vec![port(
                "control",
                &navigation_control_type(),
                PortDirection::Output,
            )],
        ),
    ]
}

pub fn install_navigation_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in navigation_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for (kind, inputs, outputs) in navigation_kind_contracts() {
        startup
            .insert(KindSignature {
                kind: kind.as_str().into(),
                startup_parameters: vec![],
            })
            .map_err(|error| error.to_string())?;
        profile
            .insert(KindDefinition {
                kind_id: kind,
                kind_contract_revision: KindContractRevision::from(NAVIGATION_REVISION),
                inputs,
                outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed navigation type is bounded")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
