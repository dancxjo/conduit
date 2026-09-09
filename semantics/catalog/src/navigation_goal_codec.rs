//! Decoding for authored or externally proposed navigation goals and time.

use conduit_core::{QuantityUnit, StructuredInfoValueShape};

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
