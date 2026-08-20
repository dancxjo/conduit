//! Deterministic gamepad, pointer, touch, button, and rotary fixtures.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    InfoBool, Quantity, QuantityDimension, QuantityUnit, StructuredFieldValue,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoValue,
};

use crate::{
    gamepad_state_type, input_axis_slot_type, input_axis_slots_type, input_axis_state_type,
    input_button_phase_type, input_button_slot_type, input_button_slots_type,
    input_button_state_type, input_button_transition_type, input_pressure_policy_type,
    input_pressure_type, point2_type, pointer_event_type, rotary_direction_type, rotary_step_type,
    touch_contact_phase_type, touch_contact_slot_type, touch_contact_type, touch_contacts_type,
    touch_frame_type, vector2_type, MAXIMUM_INPUT_AXES, MAXIMUM_INPUT_BUTTONS,
    MAXIMUM_TOUCH_CONTACTS,
};

pub const NORMALIZED_BIPOLAR_AXIS_PROFILE: &str = "input/normalized-bipolar@1";
const SURFACE_FRAME: &str = "input/surface-normalized";

pub struct GeneralizedInputFixture {
    pub button: StructuredInfoValue,
    pub gamepad: StructuredInfoValue,
    pub pointer: StructuredInfoValue,
    pub rotary: StructuredInfoValue,
    pub touch: StructuredInfoValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralizedInputRefusal {
    NonRatio,
    OutsideNormalizedRange,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for GeneralizedInputRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
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

pub fn deterministic_generalized_input_fixture(
) -> Result<GeneralizedInputFixture, GeneralizedInputRefusal> {
    let axes = fixed_slots(
        input_axis_slots_type(),
        input_axis_slot_type(),
        "axis",
        vec![
            axis_value("axis/left-x", -250_000)?,
            axis_value("axis/left-y", 750_000)?,
        ],
        usize::from(MAXIMUM_INPUT_AXES),
    )?;
    let buttons = fixed_slots(
        input_button_slots_type(),
        input_button_slot_type(),
        "button",
        vec![
            button_state_value("button/south", true)?,
            button_state_value("button/east", false)?,
        ],
        usize::from(MAXIMUM_INPUT_BUTTONS),
    )?;
    let gamepad = record_value(
        gamepad_state_type(),
        vec![
            ("axes", axes),
            ("buttons", buttons),
            (
                "pressure",
                pressure_value("coalesce_latest_state", 1, 0, 4)?,
            ),
            ("sequence", count_value(7)),
            (
                "source_profile",
                text_value("input/deterministic-gamepad@1"),
            ),
        ],
    )?;

    let pointer_buttons = fixed_slots(
        input_button_slots_type(),
        input_button_slot_type(),
        "button",
        vec![button_state_value("button/primary", true)?],
        usize::from(MAXIMUM_INPUT_BUTTONS),
    )?;
    let pointer = record_value(
        pointer_event_type(),
        vec![
            ("buttons", pointer_buttons),
            ("delta", coordinate_value(vector2_type(), 25_000, -10_000)?),
            (
                "position",
                coordinate_value(point2_type(), 400_000, 600_000)?,
            ),
            (
                "pressure",
                pressure_value("coalesce_latest_state", 2, 1, 8)?,
            ),
            ("sequence", count_value(11)),
        ],
    )?;

    let contact = record_value(
        touch_contact_type(),
        vec![
            ("contact_identity", text_value("contact/1")),
            ("phase", unit_variant(touch_contact_phase_type(), "move")?),
            (
                "position",
                coordinate_value(point2_type(), 200_000, 300_000)?,
            ),
            ("pressure", pressure_quantity_value(650_000)?),
        ],
    )?;
    let contacts = fixed_slots(
        touch_contacts_type(),
        touch_contact_slot_type(),
        "contact",
        vec![contact],
        usize::from(MAXIMUM_TOUCH_CONTACTS),
    )?;
    let touch = record_value(
        touch_frame_type(),
        vec![
            ("contacts", contacts),
            (
                "pressure",
                pressure_value("coalesce_latest_state", 3, 0, 5)?,
            ),
            ("sequence", count_value(13)),
        ],
    )?;

    let button = record_value(
        input_button_transition_type(),
        vec![
            ("button_identity", text_value("button/south")),
            ("phase", unit_variant(input_button_phase_type(), "pressed")?),
            ("sequence", count_value(8)),
        ],
    )?;
    let rotary = record_value(
        rotary_step_type(),
        vec![
            ("control_identity", text_value("rotary/menu")),
            (
                "direction",
                unit_variant(rotary_direction_type(), "clockwise")?,
            ),
            ("sequence", count_value(14)),
            ("steps", count_value(2)),
        ],
    )?;
    Ok(GeneralizedInputFixture {
        button,
        gamepad,
        pointer,
        rotary,
        touch,
    })
}

fn axis_value(
    identity: &str,
    normalized: i64,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    let value = Quantity::new(normalized, QuantityUnit::Millionth);
    validate_normalized_axis(value)?;
    record_value(
        input_axis_state_type(),
        vec![
            ("axis_identity", text_value(identity)),
            ("range_profile", text_value(NORMALIZED_BIPOLAR_AXIS_PROFILE)),
            ("value", quantity_value(value)?),
        ],
    )
}

fn button_state_value(
    identity: &str,
    pressed: bool,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    record_value(
        input_button_state_type(),
        vec![
            ("button_identity", text_value(identity)),
            ("pressed", bool_value(pressed)?),
        ],
    )
}

fn pressure_value(
    policy: &str,
    coalesced: u64,
    dropped: u64,
    queue_capacity: u64,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    record_value(
        input_pressure_type(),
        vec![
            ("coalesced", count_value(coalesced)),
            ("dropped", count_value(dropped)),
            (
                "policy",
                unit_variant(input_pressure_policy_type(), policy)?,
            ),
            ("queue_capacity", count_value(queue_capacity)),
        ],
    )
}

fn coordinate_value(
    value_type: StructuredInfoType,
    x: i64,
    y: i64,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    record_value(
        value_type,
        vec![
            ("frame", text_value(SURFACE_FRAME)),
            (
                "x",
                quantity_value(Quantity::new(x, QuantityUnit::Millionth))?,
            ),
            (
                "y",
                quantity_value(Quantity::new(y, QuantityUnit::Millionth))?,
            ),
        ],
    )
}

fn pressure_quantity_value(value: i64) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    let value = Quantity::new(value, QuantityUnit::Millionth);
    validate_normalized_pressure(value)?;
    quantity_value(value)
}

fn quantity_value(value: Quantity) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    leaf_value(conduit_core::QUANTITY_INFO_ID, value.encode().to_vec())
}

fn bool_value(value: bool) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    leaf_value(
        conduit_core::BOOL_INFO_ID,
        InfoBool::new(value).encode().to_vec(),
    )
}

fn fixed_slots(
    collection_type: StructuredInfoType,
    slot_type: StructuredInfoType,
    active_tag: &str,
    active: Vec<StructuredInfoValue>,
    length: usize,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    let mut slots = active
        .into_iter()
        .map(|value| StructuredInfoValue::variant(slot_type.clone(), active_tag, value))
        .collect::<Result<Vec<_>, _>>()?;
    while slots.len() < length {
        slots.push(unit_variant(slot_type.clone(), "unused")?);
    }
    Ok(StructuredInfoValue::collection(collection_type, slots)?)
}

fn unit_variant(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    Ok(StructuredInfoValue::variant(
        value_type,
        tag,
        leaf_value("value/unit@1", Vec::new())?,
    )?)
}

fn text_value(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/text@1")).unwrap(),
        value.as_bytes().to_vec(),
    )
    .expect("bounded deterministic input text")
}

fn count_value(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/count@1")).unwrap(),
        value.to_string().into_bytes(),
    )
    .expect("bounded deterministic input count")
}

fn leaf_value(kind: &str, bytes: Vec<u8>) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind))?,
        bytes,
    )?)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, GeneralizedInputRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}
