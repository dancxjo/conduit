//! Native Conduitese contracts for bounded non-keyboard input Info.

use alloc::{vec, vec::Vec};
use conduit_core::StructuredInfoType;
use conduit_human::{
    GamepadState, InputAxisSlot, InputAxisSlots, InputAxisState, InputButtonPhase, InputButtonSlot,
    InputButtonSlots, InputButtonState, InputButtonTransition, InputPressure, InputPressurePolicy,
    PointerEvent, RotaryDirection, RotaryStep, TouchContact, TouchContactPhase, TouchContactSlot,
    TouchContacts, TouchFrame,
};
use conduit_plot::rust_binding::NativeRustBinding;

pub const INPUT_BUTTON_TRANSITION_TYPE: &str = "InputButtonTransition";
pub const INPUT_AXIS_STATE_TYPE: &str = "InputAxisState";
pub const INPUT_AXIS_SLOTS_TYPE: &str = "InputAxisSlots";
pub const INPUT_BUTTON_SLOTS_TYPE: &str = "InputButtonSlots";
pub const POINTER_EVENT_TYPE: &str = "PointerEvent";
/// Delivery classification retained independently of semantic Type identity.
pub const POINTER_EVENT_INFO_ID: &str = "input/pointer-event@1";
pub const TOUCH_FRAME_TYPE: &str = "TouchFrame";
pub const ROTARY_STEP_TYPE: &str = "RotaryStep";
pub const GAMEPAD_STATE_TYPE: &str = "GamepadState";
pub const INPUT_PRESSURE_TYPE: &str = "InputPressure";
pub const MAXIMUM_INPUT_AXES: u16 = 4;
pub const MAXIMUM_INPUT_BUTTONS: u16 = 8;
pub const MAXIMUM_TOUCH_CONTACTS: u16 = 5;

fn native<T: NativeRustBinding>() -> StructuredInfoType {
    T::semantic_type().expect("checked generalized input Type")
}

pub fn input_pressure_policy_type() -> StructuredInfoType {
    native::<InputPressurePolicy>()
}
pub fn input_pressure_type() -> StructuredInfoType {
    native::<InputPressure>()
}
pub fn input_button_phase_type() -> StructuredInfoType {
    native::<InputButtonPhase>()
}
pub fn input_button_transition_type() -> StructuredInfoType {
    native::<InputButtonTransition>()
}
pub fn input_button_state_type() -> StructuredInfoType {
    native::<InputButtonState>()
}
pub fn input_button_slot_type() -> StructuredInfoType {
    native::<InputButtonSlot>()
}
pub fn input_button_slots_type() -> StructuredInfoType {
    native::<InputButtonSlots>()
}
pub fn input_axis_state_type() -> StructuredInfoType {
    native::<InputAxisState>()
}
pub fn input_axis_slot_type() -> StructuredInfoType {
    native::<InputAxisSlot>()
}
pub fn input_axis_slots_type() -> StructuredInfoType {
    native::<InputAxisSlots>()
}
pub fn pointer_event_type() -> StructuredInfoType {
    native::<PointerEvent>()
}
pub fn touch_contact_phase_type() -> StructuredInfoType {
    native::<TouchContactPhase>()
}
pub fn touch_contact_type() -> StructuredInfoType {
    native::<TouchContact>()
}
pub fn touch_contact_slot_type() -> StructuredInfoType {
    native::<TouchContactSlot>()
}
pub fn touch_contacts_type() -> StructuredInfoType {
    native::<TouchContacts>()
}
pub fn touch_frame_type() -> StructuredInfoType {
    native::<TouchFrame>()
}
pub fn rotary_direction_type() -> StructuredInfoType {
    native::<RotaryDirection>()
}
pub fn rotary_step_type() -> StructuredInfoType {
    native::<RotaryStep>()
}
pub fn gamepad_state_type() -> StructuredInfoType {
    native::<GamepadState>()
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
