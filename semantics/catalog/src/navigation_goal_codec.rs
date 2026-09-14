//! Decoding for authored or externally proposed navigation goals and time.

use alloc::{vec, vec::Vec};
use conduit_core::{QuantityUnit, StructuredInfoValue, StructuredInfoValueShape};

use crate::navigation_codec_support::*;
use crate::{
    navigation_goal_type, navigation_time_type, GoalTarget, NavigationCodecError, NavigationGoal,
    NavigationTime,
};

pub fn decode_navigation_time(encoded: &[u8]) -> Result<NavigationTime, NavigationCodecError> {
    let value = exact(encoded, &navigation_time_type())?;
    Ok(NavigationTime {
        clock_identity: text(record_field(&value, "clock_identity")?)?,
        now_ms: u64_quantity(record_field(&value, "now")?, QuantityUnit::Millisecond)?,
    })
}

pub fn decode_navigation_goal(encoded: &[u8]) -> Result<NavigationGoal, NavigationCodecError> {
    let value = exact(encoded, &navigation_goal_type())?;
    let validity = validity(record_field(&value, "validity")?)?;
    let target = record_field(&value, "target")?;
    let StructuredInfoValueShape::Variant { tag, payload } = target.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    let target = match tag {
        "hold" => GoalTarget::Hold,
        "reach" => {
            let pose = record_field(payload, "pose")?;
            let position = record_field(pose, "position")?;
            GoalTarget::Reach {
                frame: text(record_field(position, "frame")?)?,
                x_mm: i32_quantity(record_field(position, "x")?, QuantityUnit::Millimeter)?,
                y_mm: i32_quantity(record_field(position, "y")?, QuantityUnit::Millimeter)?,
                heading_microdegrees: i32_quantity(
                    record_field(pose, "heading")?,
                    QuantityUnit::Microdegree,
                )?,
                position_tolerance_mm: u32_quantity(
                    record_field(payload, "position_tolerance")?,
                    QuantityUnit::Millimeter,
                )?,
                heading_tolerance_microdegrees: u32_quantity(
                    record_field(payload, "heading_tolerance")?,
                    QuantityUnit::Microdegree,
                )?,
            }
        }
        _ => return Err(NavigationCodecError::Malformed),
    };
    Ok(NavigationGoal {
        identity: text(record_field(&value, "goal_identity")?)?,
        clock_identity: validity.0,
        valid_until_ms: validity.1,
        target,
    })
}

pub fn encode_navigation_time(value: &NavigationTime) -> Result<Vec<u8>, NavigationCodecError> {
    Ok(record_value(
        navigation_time_type(),
        vec![
            ("clock_identity", text_value(&value.clock_identity)?),
            (
                "now",
                quantity_value(
                    value
                        .now_ms
                        .try_into()
                        .map_err(|_| NavigationCodecError::InexactQuantity)?,
                    QuantityUnit::Millisecond,
                )?,
            ),
        ],
    )?
    .canonical_bytes()?)
}

pub fn encode_navigation_goal(value: &NavigationGoal) -> Result<Vec<u8>, NavigationCodecError> {
    let ty = navigation_goal_type();
    let target_ty = field_type(&ty, "target")?;
    let (tag, payload) = match &value.target {
        GoalTarget::Hold => {
            let unit = variant_type(&target_ty, "hold")?;
            ("hold", StructuredInfoValue::leaf(unit, Vec::new())?)
        }
        GoalTarget::Reach {
            frame,
            x_mm,
            y_mm,
            heading_microdegrees,
            position_tolerance_mm,
            heading_tolerance_microdegrees,
        } => {
            let reach = variant_type(&target_ty, "reach")?;
            let pose_ty = field_type(&reach, "pose")?;
            let pose = record_value(
                pose_ty,
                vec![
                    (
                        "heading",
                        quantity_value((*heading_microdegrees).into(), QuantityUnit::Microdegree)?,
                    ),
                    (
                        "position",
                        conduit_presentation::point2_value(
                            frame,
                            conduit_core::Quantity::new((*x_mm).into(), QuantityUnit::Millimeter),
                            conduit_core::Quantity::new((*y_mm).into(), QuantityUnit::Millimeter),
                        )
                        .map_err(|_| NavigationCodecError::Malformed)?,
                    ),
                ],
            )?;
            (
                "reach",
                record_value(
                    reach,
                    vec![
                        (
                            "heading_tolerance",
                            quantity_value(
                                (*heading_tolerance_microdegrees).into(),
                                QuantityUnit::Microdegree,
                            )?,
                        ),
                        ("pose", pose),
                        (
                            "position_tolerance",
                            quantity_value(
                                (*position_tolerance_mm).into(),
                                QuantityUnit::Millimeter,
                            )?,
                        ),
                    ],
                )?,
            )
        }
    };
    let target = StructuredInfoValue::variant(target_ty, tag, payload)?;
    let validity_ty = field_type(&ty, "validity")?;
    let validity = record_value(
        validity_ty,
        vec![
            ("clock_identity", text_value(&value.clock_identity)?),
            (
                "valid_until",
                quantity_value(
                    value
                        .valid_until_ms
                        .try_into()
                        .map_err(|_| NavigationCodecError::InexactQuantity)?,
                    QuantityUnit::Millisecond,
                )?,
            ),
        ],
    )?;
    Ok(record_value(
        ty,
        vec![
            ("goal_identity", text_value(&value.identity)?),
            ("target", target),
            ("validity", validity),
        ],
    )?
    .canonical_bytes()?)
}
