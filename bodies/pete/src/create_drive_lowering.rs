//! Exact lowering from portable body velocity intent to Create wheel demand.

use crate::{DifferentialMotionRequest, CREATE_OI_MAX_WHEEL_SPEED_MM_S};
use conduit_core::Scalar;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateDriveLoweringRefusal {
    CombinedIntentOverflow,
    WheelDemandOutsideRealization,
}

/// Lowers normalized portable intent without clipping.
///
/// Positive angular intent is counter-clockwise: the right wheel advances
/// relative to the left. One scalar unit maps to the exact Create wheel-speed
/// limit. A combined demand beyond one unit is refused rather than distorted.
pub fn lower_create_drive_intent(
    linear: Scalar,
    angular: Scalar,
    ttl_ms: u32,
) -> Result<DifferentialMotionRequest, CreateDriveLoweringRefusal> {
    let linear = linear.raw_microunits();
    let angular = angular.raw_microunits();
    let left = linear
        .checked_sub(angular)
        .ok_or(CreateDriveLoweringRefusal::CombinedIntentOverflow)?;
    let right = linear
        .checked_add(angular)
        .ok_or(CreateDriveLoweringRefusal::CombinedIntentOverflow)?;
    let limit = Scalar::SCALE;
    if left.unsigned_abs() > limit as u64 || right.unsigned_abs() > limit as u64 {
        return Err(CreateDriveLoweringRefusal::WheelDemandOutsideRealization);
    }
    Ok(DifferentialMotionRequest {
        left_mm_s: wheel_speed(left),
        right_mm_s: wheel_speed(right),
        ttl_ms,
    })
}

fn wheel_speed(raw_microunits: i64) -> i16 {
    let scaled = i128::from(raw_microunits) * i128::from(CREATE_OI_MAX_WHEEL_SPEED_MM_S)
        / i128::from(Scalar::SCALE);
    i16::try_from(scaled).expect("admitted normalized Create wheel demand fits i16")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(raw: i64) -> Scalar {
        Scalar::from_raw_microunits(raw)
    }

    #[test]
    fn exact_body_intent_maps_to_create_wheels_without_clipping() {
        assert_eq!(
            lower_create_drive_intent(scalar(500_000), scalar(250_000), 100),
            Ok(DifferentialMotionRequest {
                left_mm_s: 125,
                right_mm_s: 375,
                ttl_ms: 100,
            })
        );
        assert_eq!(
            lower_create_drive_intent(Scalar::ZERO, Scalar::ONE, 100),
            Ok(DifferentialMotionRequest {
                left_mm_s: -500,
                right_mm_s: 500,
                ttl_ms: 100,
            })
        );
    }

    #[test]
    fn overflow_and_unrealizable_combination_refuse() {
        assert_eq!(
            lower_create_drive_intent(Scalar::MAX, scalar(-1), 100),
            Err(CreateDriveLoweringRefusal::CombinedIntentOverflow)
        );
        assert_eq!(
            lower_create_drive_intent(scalar(750_000), scalar(500_000), 100),
            Err(CreateDriveLoweringRefusal::WheelDemandOutsideRealization)
        );
    }
}
