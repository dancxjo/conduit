//! Portable bounded non-keyboard input Info.
//!
//! Device reports, DOM events, Bluetooth, USB, HID codes, and window identity
//! remain realization facts below these semantic values.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoType, StructuredVariantCase, BOOL_INFO_ID,
    QUANTITY_INFO_ID,
};

use crate::{point2_type, vector2_type};

pub const INPUT_BUTTON_TRANSITION_TYPE: &str = "InputButtonTransition";
pub const INPUT_AXIS_STATE_TYPE: &str = "InputAxisState";
pub const INPUT_AXIS_SLOTS_TYPE: &str = "InputAxisSlots";
pub const INPUT_BUTTON_SLOTS_TYPE: &str = "InputButtonSlots";
pub const POINTER_EVENT_TYPE: &str = "PointerEvent";
pub const TOUCH_FRAME_TYPE: &str = "TouchFrame";
pub const ROTARY_STEP_TYPE: &str = "RotaryStep";
pub const GAMEPAD_STATE_TYPE: &str = "GamepadState";
pub const INPUT_PRESSURE_TYPE: &str = "InputPressure";
pub const MAXIMUM_INPUT_AXES: u16 = 4;
pub const MAXIMUM_INPUT_BUTTONS: u16 = 8;
pub const MAXIMUM_TOUCH_CONTACTS: u16 = 5;

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed input leaf")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count@1")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed input field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed input case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed input record")
}

pub fn input_pressure_policy_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("input/pressure-policy@1"),
        vec![
            case("coalesce_latest_state", unit_type()),
            case("ordered_transitions", unit_type()),
        ],
    )
    .expect("reviewed input pressure policy")
}

pub fn input_pressure_type() -> StructuredInfoType {
    record(
        "input/pressure@1",
        vec![
            field("coalesced", count_type()),
            field("dropped", count_type()),
            field("policy", input_pressure_policy_type()),
            field("queue_capacity", count_type()),
        ],
    )
}

pub fn input_button_phase_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("input/button-phase@1"),
        vec![case("pressed", unit_type()), case("released", unit_type())],
    )
    .expect("reviewed button phase")
}

pub fn input_button_transition_type() -> StructuredInfoType {
    record(
        "input/button-transition@1",
        vec![
            field("button_identity", text_type()),
            field("phase", input_button_phase_type()),
            field("sequence", count_type()),
        ],
    )
}

pub fn input_button_state_type() -> StructuredInfoType {
    record(
        "input/button-state@1",
        vec![
            field("button_identity", text_type()),
            field("pressed", leaf(BOOL_INFO_ID)),
        ],
    )
}

pub fn input_button_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("input/button-slot@1"),
        vec![
            case("button", input_button_state_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed button slot")
}

pub fn input_button_slots_type() -> StructuredInfoType {
    StructuredInfoType::collection(input_button_slot_type(), Some(MAXIMUM_INPUT_BUTTONS))
        .expect("fixed input button slots")
}

pub fn input_axis_state_type() -> StructuredInfoType {
    record(
        "input/axis-state@1",
        vec![
            field("axis_identity", text_type()),
            field("range_profile", text_type()),
            field("value", leaf(QUANTITY_INFO_ID)),
        ],
    )
}

pub fn input_axis_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("input/axis-slot@1"),
        vec![
            case("axis", input_axis_state_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed axis slot")
}

pub fn input_axis_slots_type() -> StructuredInfoType {
    StructuredInfoType::collection(input_axis_slot_type(), Some(MAXIMUM_INPUT_AXES))
        .expect("fixed input axis slots")
}

pub fn pointer_event_type() -> StructuredInfoType {
    record(
        "input/pointer-event@1",
        vec![
            field("buttons", input_button_slots_type()),
            field("delta", vector2_type()),
            field("position", point2_type()),
            field("pressure", input_pressure_type()),
            field("sequence", count_type()),
        ],
    )
}

pub fn touch_contact_phase_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("input/touch-phase@1"),
        vec![
            case("begin", unit_type()),
            case("end", unit_type()),
            case("move", unit_type()),
        ],
    )
    .expect("reviewed touch phase")
}

pub fn touch_contact_type() -> StructuredInfoType {
    record(
        "input/touch-contact@1",
        vec![
            field("contact_identity", text_type()),
            field("phase", touch_contact_phase_type()),
            field("position", point2_type()),
            field("pressure", leaf(QUANTITY_INFO_ID)),
        ],
    )
}

pub fn touch_contact_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("input/touch-contact-slot@1"),
        vec![
            case("contact", touch_contact_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed touch contact slot")
}

pub fn touch_contacts_type() -> StructuredInfoType {
    StructuredInfoType::collection(touch_contact_slot_type(), Some(MAXIMUM_TOUCH_CONTACTS))
        .expect("fixed touch contact slots")
}

pub fn touch_frame_type() -> StructuredInfoType {
    record(
        "input/touch-frame@1",
        vec![
            field("contacts", touch_contacts_type()),
            field("pressure", input_pressure_type()),
            field("sequence", count_type()),
        ],
    )
}

pub fn rotary_direction_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("input/rotary-direction@1"),
        vec![
            case("clockwise", unit_type()),
            case("counterclockwise", unit_type()),
        ],
    )
    .expect("reviewed rotary direction")
}

pub fn rotary_step_type() -> StructuredInfoType {
    record(
        "input/rotary-step@1",
        vec![
            field("control_identity", text_type()),
            field("direction", rotary_direction_type()),
            field("sequence", count_type()),
            field("steps", count_type()),
        ],
    )
}

pub fn gamepad_state_type() -> StructuredInfoType {
    record(
        "input/gamepad-state@1",
        vec![
            field("axes", input_axis_slots_type()),
            field("buttons", input_button_slots_type()),
            field("pressure", input_pressure_type()),
            field("sequence", count_type()),
            field("source_profile", text_type()),
        ],
    )
}

pub fn generalized_input_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (INPUT_BUTTON_TRANSITION_TYPE, input_button_transition_type()),
        (INPUT_AXIS_STATE_TYPE, input_axis_state_type()),
        (INPUT_AXIS_SLOTS_TYPE, input_axis_slots_type()),
        (INPUT_BUTTON_SLOTS_TYPE, input_button_slots_type()),
        (POINTER_EVENT_TYPE, pointer_event_type()),
        (TOUCH_FRAME_TYPE, touch_frame_type()),
        (ROTARY_STEP_TYPE, rotary_step_type()),
        (GAMEPAD_STATE_TYPE, gamepad_state_type()),
        (INPUT_PRESSURE_TYPE, input_pressure_type()),
    ]
}
