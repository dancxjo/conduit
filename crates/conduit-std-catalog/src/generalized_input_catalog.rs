//! Canonical Forms and deterministic hosted offers for generalized input Info.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    gamepad_state_type, generalized_input_registered_types, input_button_transition_type,
    pointer_event_type, rotary_step_type, touch_frame_type,
};

pub const DETERMINISTIC_GAMEPAD_KIND: &str = "input/deterministic-gamepad";
pub const DETERMINISTIC_POINTER_TOUCH_KIND: &str = "input/deterministic-pointer-touch";
pub const GENERALIZED_INPUT_REVISION: &str = "conduit.std/generalized-input@1";
pub const GENERALIZED_INPUT_PROFILE: &str = "std/generalized-input-deterministic@1";
pub const GENERALIZED_INPUT_ARTIFACT: &str = "conduit-std-host/generalized-input@1";
pub const GENERALIZED_INPUT_HOST_OPERATION: &str = "conduit.host/generalized-input@1";

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
        DETERMINISTIC_GAMEPAD_KIND,
        vec![
            port(
                "button",
                &input_button_transition_type(),
                PortDirection::Output,
            ),
            port("gamepad", &gamepad_state_type(), PortDirection::Output),
            port("rotary", &rotary_step_type(), PortDirection::Output),
        ],
    )?;
    insert_kind(
        startup,
        profile,
        DETERMINISTIC_POINTER_TOUCH_KIND,
        vec![
            port("pointer", &pointer_event_type(), PortDirection::Output),
            port("touch", &touch_frame_type(), PortDirection::Output),
        ],
    )
}

pub fn generalized_input_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            DETERMINISTIC_GAMEPAD_KIND,
            vec![
                port(
                    "button",
                    &input_button_transition_type(),
                    PortDirection::Output,
                ),
                port("gamepad", &gamepad_state_type(), PortDirection::Output),
                port("rotary", &rotary_step_type(), PortDirection::Output),
            ],
        ),
        offer(
            DETERMINISTIC_POINTER_TOUCH_KIND,
            vec![
                port("pointer", &pointer_event_type(), PortDirection::Output),
                port("touch", &touch_frame_type(), PortDirection::Output),
            ],
        ),
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

fn offer(kind: &str, outputs: Vec<PortDescriptor>) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(GENERALIZED_INPUT_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(GENERALIZED_INPUT_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(GENERALIZED_INPUT_ARTIFACT),
        },
        inputs: vec![],
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(GENERALIZED_INPUT_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 8,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
        },
    }
}
