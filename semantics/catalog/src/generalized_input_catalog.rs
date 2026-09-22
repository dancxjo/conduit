//! Canonical Forms for generalized input Info.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindProjection, KindSignature};

use crate::{
    gamepad_state_type, generalized_input_registered_types, input_button_transition_type,
    pointer_event_type, rotary_step_type, touch_frame_type,
};

pub const DETERMINISTIC_GAMEPAD_KIND: &str = "input/deterministic-gamepad";
pub const DETERMINISTIC_POINTER_TOUCH_KIND: &str = "input/deterministic-pointer-touch";
pub const POINTER_SOURCE_KIND: &str = "input/pointer-source";
pub const GENERALIZED_INPUT_REVISION: &str = "conduit.std/generalized-input@2";

pub fn install_generalized_input_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in generalized_input_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_semantic_kind(startup, profile, pointer_source_semantic_contract())?;
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

pub fn deterministic_gamepad_semantic_contract() -> Kind {
    semantic_contract(DETERMINISTIC_GAMEPAD_KIND, deterministic_gamepad_outputs())
}

pub fn deterministic_pointer_touch_semantic_contract() -> Kind {
    semantic_contract(
        DETERMINISTIC_POINTER_TOUCH_KIND,
        deterministic_pointer_touch_outputs(),
    )
}

pub fn pointer_source_semantic_contract() -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(POINTER_SOURCE_KIND),
        kind_contract_revision: KindIdentity::from(GENERALIZED_INPUT_REVISION),
        inputs: vec![],
        outputs: vec![source_port(
            "pointer",
            &pointer_event_type(),
            PortDirection::Output,
        )],
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}

fn semantic_contract(kind: &str, outputs: Vec<PortDescriptor>) -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(GENERALIZED_INPUT_REVISION),
        inputs: vec![],
        outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 8,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
        },
    }
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    insert_semantic_kind(startup, profile, semantic_contract(kind, outputs))
}

fn insert_semantic_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    contract: Kind,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
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

fn source_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    let mut port = port(name, value_type, direction);
    port.temporal = PortTemporal::Flow { closes: false };
    port
}
