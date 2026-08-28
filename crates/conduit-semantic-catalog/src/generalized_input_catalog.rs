//! Canonical Forms for generalized input Info.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    gamepad_state_type, generalized_input_registered_types, input_button_transition_type,
    pointer_event_type, rotary_step_type, touch_frame_type,
};

pub const DETERMINISTIC_GAMEPAD_KIND: &str = "input/deterministic-gamepad";
pub const DETERMINISTIC_POINTER_TOUCH_KIND: &str = "input/deterministic-pointer-touch";
pub const POINTER_SOURCE_KIND: &str = "input/pointer-source";
pub const GENERALIZED_INPUT_REVISION: &str = "conduit.std/generalized-input@1";

pub fn install_generalized_input_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in generalized_input_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_kind(
        startup,
        profile,
        POINTER_SOURCE_KIND,
        vec![port(
            "pointer",
            &pointer_event_type(),
            PortDirection::Output,
        )],
    )?;
    insert_kind(
        startup,
        profile,
        DETERMINISTIC_GAMEPAD_KIND,
        deterministic_gamepad_outputs(),
    )?;
    insert_kind(
        startup,
        profile,
        DETERMINISTIC_POINTER_TOUCH_KIND,
        deterministic_pointer_touch_outputs(),
    )
}

pub fn deterministic_gamepad_outputs() -> Vec<PortDescriptor> {
    vec![
        port(
            "button",
            &input_button_transition_type(),
            PortDirection::Output,
        ),
        port("gamepad", &gamepad_state_type(), PortDirection::Output),
        port("rotary", &rotary_step_type(), PortDirection::Output),
    ]
}

pub fn deterministic_pointer_touch_outputs() -> Vec<PortDescriptor> {
    vec![
        port("pointer", &pointer_event_type(), PortDirection::Output),
        port("touch", &touch_frame_type(), PortDirection::Output),
    ]
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
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
            kind_contract_revision: KindContractRevision::from(GENERALIZED_INPUT_REVISION),
            inputs: vec![],
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
            .expect("reviewed generalized input profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
