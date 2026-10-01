//! Deterministic gamepad, pointer, touch, button, and rotary fixtures.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    Quantity, QuantityDimension, QuantityUnit, StructuredInfoRefusal, StructuredInfoValue,
};
use conduit_form::rust_binding::NativeRustBinding;
use conduit_human::{
    GamepadState, InputAxisSlot, InputAxisSlots, InputButtonPhase, InputButtonSlot,
    InputButtonSlots, InputButtonTransition, InputPressure, InputPressurePolicy, InputSurfacePoint,
    InputSurfaceVector, PointerEvent, RotaryDirection, RotaryStep, TouchContactPhase,
    TouchContactSlot, TouchContacts, TouchFrame,
};

use crate::{MAXIMUM_INPUT_AXES, MAXIMUM_INPUT_BUTTONS, MAXIMUM_TOUCH_CONTACTS};

pub const NORMALIZED_BIPOLAR_AXIS_PROFILE: &str = "input/normalized-bipolar@1";
const SURFACE_FRAME: &str = "input/surface-normalized";

pub struct GeneralizedInputFixture {
    pub button: StructuredInfoValue,
    pub gamepad: StructuredInfoValue,
    pub pointer: StructuredInfoValue,
    pub rotary: StructuredInfoValue,
    pub touch: StructuredInfoValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedPointerSample {
    pub position_x: i64,
    pub position_y: i64,
    pub delta_x: i64,
    pub delta_y: i64,
    pub primary_pressed: bool,
    pub coalesced: u64,
    pub dropped: u64,
    pub queue_capacity: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralizedInputRefusal {
    NonRatio,
    OutsideNormalizedRange,
    InvalidPressureBound,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for GeneralizedInputRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

fn malformed<T>(_: T) -> GeneralizedInputRefusal {
    GeneralizedInputRefusal::Structured(StructuredInfoRefusal::WrongType)
}

fn structured<T: NativeRustBinding>(
    value: T,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    value.into_structured().map_err(malformed)
}

pub fn validate_normalized_axis(value: Quantity) -> Result<(), GeneralizedInputRefusal> {
    validate_ratio(value, -1_000_000, 1_000_000)
}

pub fn validate_normalized_pressure(value: Quantity) -> Result<(), GeneralizedInputRefusal> {
    validate_ratio(value, 0, 1_000_000)
}

fn validate_ratio(
    value: Quantity,
    minimum: i64,
    maximum: i64,
) -> Result<(), GeneralizedInputRefusal> {
    if value.dimension() != QuantityDimension::Ratio {
        return Err(GeneralizedInputRefusal::NonRatio);
    }
    let normalized = value
        .convert(QuantityUnit::Millionth)
        .map_err(|_| GeneralizedInputRefusal::OutsideNormalizedRange)?;
    if !(minimum..=maximum).contains(&normalized.value()) {
        return Err(GeneralizedInputRefusal::OutsideNormalizedRange);
    }
    Ok(())
}

pub fn normalized_pointer_value(
    sample: NormalizedPointerSample,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    for coordinate in [sample.position_x, sample.position_y] {
        validate_normalized_pressure(ratio(coordinate))?;
    }
    for delta in [sample.delta_x, sample.delta_y] {
        validate_normalized_axis(ratio(delta))?;
    }
    if sample.queue_capacity == 0 {
        return Err(GeneralizedInputRefusal::InvalidPressureBound);
    }
    let buttons = button_slots(vec![InputButtonSlot::button(
        "button/primary".to_string(),
        sample.primary_pressed,
    )
    .map_err(malformed)?])?;
    structured(
        PointerEvent::new(
            buttons,
            InputSurfaceVector::new(
                SURFACE_FRAME.to_string(),
                ratio(sample.delta_x),
                ratio(sample.delta_y),
            )
            .map_err(malformed)?,
            InputSurfacePoint::new(
                SURFACE_FRAME.to_string(),
                ratio(sample.position_x),
                ratio(sample.position_y),
            )
            .map_err(malformed)?,
            pressure(
                InputPressurePolicy::CoalesceLatestState,
                sample.coalesced,
                sample.dropped,
                sample.queue_capacity,
            )?,
            sample.sequence,
        )
        .map_err(malformed)?,
    )
}

pub fn deterministic_generalized_input_fixture(
) -> Result<GeneralizedInputFixture, GeneralizedInputRefusal> {
    let axes = axis_slots(vec![
        axis("axis/left-x", -250_000)?,
        axis("axis/left-y", 750_000)?,
    ])?;
    let buttons = button_slots(vec![
        button("button/south", true)?,
        button("button/east", false)?,
    ])?;
    let gamepad = structured(
        GamepadState::new(
            axes,
            buttons,
            pressure(InputPressurePolicy::CoalesceLatestState, 1, 0, 4)?,
            7,
            "input/deterministic-gamepad@1".to_string(),
        )
        .map_err(malformed)?,
    )?;
    let pointer = normalized_pointer_value(NormalizedPointerSample {
        position_x: 400_000,
        position_y: 600_000,
        delta_x: 25_000,
        delta_y: -10_000,
        primary_pressed: true,
        coalesced: 2,
        dropped: 1,
        queue_capacity: 8,
        sequence: 11,
    })?;
    let contacts = touch_slots(vec![TouchContactSlot::contact(
        "contact/1".to_string(),
        TouchContactPhase::Moving,
        InputSurfacePoint::new(SURFACE_FRAME.to_string(), ratio(200_000), ratio(300_000))
            .map_err(malformed)?,
        ratio(650_000),
    )
    .map_err(malformed)?])?;
    let touch = structured(
        TouchFrame::new(
            contacts,
            pressure(InputPressurePolicy::CoalesceLatestState, 3, 0, 5)?,
            13,
        )
        .map_err(malformed)?,
    )?;
    let button = structured(
        InputButtonTransition::new("button/south".to_string(), InputButtonPhase::Pressed, 8)
            .map_err(malformed)?,
    )?;
    let rotary = structured(
        RotaryStep::new("rotary/menu".to_string(), RotaryDirection::Clockwise, 14, 2)
            .map_err(malformed)?,
    )?;
    Ok(GeneralizedInputFixture {
        button,
        gamepad,
        pointer,
        rotary,
        touch,
    })
}

fn ratio(value: i64) -> Quantity {
    Quantity::new(value, QuantityUnit::Millionth)
}

fn axis(identity: &str, normalized: i64) -> Result<InputAxisSlot, GeneralizedInputRefusal> {
    let value = ratio(normalized);
    validate_normalized_axis(value)?;
    InputAxisSlot::axis(
        identity.to_string(),
        NORMALIZED_BIPOLAR_AXIS_PROFILE.to_string(),
        value,
    )
    .map_err(malformed)
}

fn button(identity: &str, pressed: bool) -> Result<InputButtonSlot, GeneralizedInputRefusal> {
    InputButtonSlot::button(identity.to_string(), pressed).map_err(malformed)
}

fn pressure(
    policy: InputPressurePolicy,
    coalesced: u64,
    dropped: u64,
    queue_capacity: u64,
) -> Result<InputPressure, GeneralizedInputRefusal> {
    if queue_capacity == 0 {
        return Err(GeneralizedInputRefusal::InvalidPressureBound);
    }
    InputPressure::new(coalesced, dropped, policy, queue_capacity).map_err(malformed)
}

fn axis_slots(mut active: Vec<InputAxisSlot>) -> Result<InputAxisSlots, GeneralizedInputRefusal> {
    while active.len() < usize::from(MAXIMUM_INPUT_AXES) {
        active.push(InputAxisSlot::unused());
    }
    InputAxisSlots::new(
        active
            .try_into()
            .map_err(|_| GeneralizedInputRefusal::OutsideNormalizedRange)?,
    )
    .map_err(malformed)
}

fn button_slots(
    mut active: Vec<InputButtonSlot>,
) -> Result<InputButtonSlots, GeneralizedInputRefusal> {
    while active.len() < usize::from(MAXIMUM_INPUT_BUTTONS) {
        active.push(InputButtonSlot::unused());
    }
    InputButtonSlots::new(
        active
            .try_into()
            .map_err(|_| GeneralizedInputRefusal::OutsideNormalizedRange)?,
    )
    .map_err(malformed)
}

fn touch_slots(
    mut active: Vec<TouchContactSlot>,
) -> Result<TouchContacts, GeneralizedInputRefusal> {
    while active.len() < usize::from(MAXIMUM_TOUCH_CONTACTS) {
        active.push(TouchContactSlot::unused());
    }
    TouchContacts::new(
        active
            .try_into()
            .map_err(|_| GeneralizedInputRefusal::OutsideNormalizedRange)?,
    )
    .map_err(malformed)
}
