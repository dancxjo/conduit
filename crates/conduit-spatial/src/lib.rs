//! Exact bounded spatial values and deterministic first-proof operations.
//!
//! This domain package owns coordinate, transform, and calibration semantics.
//! It does not define robots, commands, a world frame, ambient transform trees,
//! ROS identities, or another scheduler/event model.

mod runtime_nodes;

pub use runtime_nodes::{
    APPLY_CONTRACT, COMPOSE_CONTRACT, INTERPOLATE_CONTRACT, INVERT_CONTRACT, PIXEL_DESCRIPTOR,
    PIXEL_TYPE, POINT_DESCRIPTOR, POINT_INSPECT_CONTRACT, POINT_LITERAL_CONTRACT, POINT_TYPE,
    PROJECT_CONTRACT, SPATIAL_CONTRACTS, TRANSFORM_DESCRIPTOR, TRANSFORM_LITERAL_CONTRACT,
    TRANSFORM_TYPE, UNPROJECT_CONTRACT, register_deterministic_spatial_provider,
    register_spatial_contracts,
};

pub const MAXIMUM_FRAME_ID_BYTES: usize = 64;
pub const MAXIMUM_TRANSFORM_EDGES: usize = 16;
pub const MAXIMUM_HISTORY_VALUES: usize = 8;
pub const MAXIMUM_NUMERIC_WORK: usize = 256;

pub const CALIBRATION_IDENTITY: [u8; 32] = [0x51; 32];
pub const PROVENANCE_IDENTITY: [u8; 32] = [0x52; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialReason {
    EmptyFrame,
    FrameTooLong,
    SameFrame,
    UnknownFrame,
    WrongFrame,
    FrameCycle,
    UnitMismatch,
    HandednessMismatch,
    AxisMismatch,
    InvalidQuaternion,
    SingularTransform,
    StaleTransform,
    ClockMismatch,
    MissingClockConversion,
    ExcessiveUncertainty,
    InterpolationBoundary,
    CalibrationMismatch,
    InvalidCalibration,
    HistoryOverflow,
    WorkOverflow,
    NumericOverflow,
    BehindCamera,
    Cancellation,
    UnsupportedProvider,
}

impl SpatialReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyFrame
            | Self::FrameTooLong
            | Self::SameFrame
            | Self::UnknownFrame
            | Self::WrongFrame
            | Self::FrameCycle => "CND-SPATIAL-001",
            Self::UnitMismatch | Self::HandednessMismatch | Self::AxisMismatch => "CND-SPATIAL-002",
            Self::InvalidQuaternion
            | Self::SingularTransform
            | Self::NumericOverflow
            | Self::BehindCamera => "CND-SPATIAL-003",
            Self::StaleTransform | Self::InterpolationBoundary => "CND-SPATIAL-004",
            Self::ClockMismatch | Self::MissingClockConversion => "CND-SPATIAL-005",
            Self::ExcessiveUncertainty => "CND-SPATIAL-006",
            Self::CalibrationMismatch | Self::InvalidCalibration => "CND-SPATIAL-007",
            Self::HistoryOverflow | Self::WorkOverflow => "CND-SPATIAL-008",
            Self::Cancellation => "CND-SPATIAL-009",
            Self::UnsupportedProvider => "CND-SPATIAL-010",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinearUnit {
    Micrometre,
    Millimetre,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Handedness {
    Right,
    Left,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisConvention {
    XRightYForwardZUp,
    XForwardYLeftZUp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumericProfile {
    pub unit: LinearUnit,
    pub handedness: Handedness,
    pub axes: AxisConvention,
    pub maximum_work: usize,
}

impl NumericProfile {
    pub const FIRST_PROOF: Self = Self {
        unit: LinearUnit::Micrometre,
        handedness: Handedness::Right,
        axes: AxisConvention::XRightYForwardZUp,
        maximum_work: 256,
    };

    fn validate(self) -> Result<(), SpatialReason> {
        if self.maximum_work == 0 || self.maximum_work > MAXIMUM_NUMERIC_WORK {
            return Err(SpatialReason::WorkOverflow);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameIdentity {
    pub id: String,
    pub unit: LinearUnit,
    pub handedness: Handedness,
    pub axes: AxisConvention,
}

impl FrameIdentity {
    pub fn new(id: impl Into<String>) -> Result<Self, SpatialReason> {
        let id = id.into();
        if id.is_empty() {
            return Err(SpatialReason::EmptyFrame);
        }
        if id.len() > MAXIMUM_FRAME_ID_BYTES {
            return Err(SpatialReason::FrameTooLong);
        }
        Ok(Self {
            id,
            unit: LinearUnit::Micrometre,
            handedness: Handedness::Right,
            axes: AxisConvention::XRightYForwardZUp,
        })
    }

    fn compatible(&self, other: &Self) -> Result<(), SpatialReason> {
        if self.unit != other.unit {
            return Err(SpatialReason::UnitMismatch);
        }
        if self.handedness != other.handedness {
            return Err(SpatialReason::HandednessMismatch);
        }
        if self.axes != other.axes {
            return Err(SpatialReason::AxisMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuaternionQ30 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
}

impl QuaternionQ30 {
    pub const IDENTITY: Self = Self {
        x: 0,
        y: 0,
        z: 0,
        w: 1 << 30,
    };

    #[must_use]
    pub const fn quarter_turn_z(turns: u8) -> Option<Self> {
        const SQRT_HALF_Q30: i32 = 759_250_125;
        match turns {
            0 => Some(Self::IDENTITY),
            1 => Some(Self {
                x: 0,
                y: 0,
                z: SQRT_HALF_Q30,
                w: SQRT_HALF_Q30,
            }),
            2 => Some(Self {
                x: 0,
                y: 0,
                z: 1 << 30,
                w: 0,
            }),
            3 => Some(Self {
                x: 0,
                y: 0,
                z: -SQRT_HALF_Q30,
                w: SQRT_HALF_Q30,
            }),
            _ => None,
        }
    }

    pub fn validate(self) -> Result<(), SpatialReason> {
        let norm =
            [self.x, self.y, self.z, self.w]
                .into_iter()
                .try_fold(0_i128, |sum, value| {
                    sum.checked_add(i128::from(value) * i128::from(value))
                        .ok_or(SpatialReason::NumericOverflow)
                })?;
        let expected = 1_i128 << 60;
        let tolerance = 1_i128 << 35;
        if norm.abs_diff(expected) > tolerance as u128 {
            return Err(SpatialReason::InvalidQuaternion);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Uncertainty {
    pub translation_um: u64,
    pub rotation_q30: u32,
    pub covariance_diagonal: [u64; 6],
}

impl Uncertainty {
    pub const EXACT: Self = Self {
        translation_um: 0,
        rotation_q30: 0,
        covariance_diagonal: [0; 6],
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Validity {
    pub clock: String,
    pub stamp_tick: u64,
    pub valid_from_tick: u64,
    pub valid_until_tick: u64,
}

impl Validity {
    fn validate(&self) -> Result<(), SpatialReason> {
        if self.clock.is_empty()
            || self.valid_from_tick > self.stamp_tick
            || self.stamp_tick > self.valid_until_tick
        {
            return Err(SpatialReason::StaleTransform);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transform3 {
    pub source: FrameIdentity,
    pub target: FrameIdentity,
    pub translation_um: [i64; 3],
    pub rotation: QuaternionQ30,
    /// The deterministic proof supports exact quarter turns around +Z.
    pub quarter_turns_z: u8,
    pub validity: Validity,
    pub uncertainty: Uncertainty,
    pub calibration_identity: [u8; 32],
    pub provenance_identity: [u8; 32],
}

impl Transform3 {
    pub fn validate(&self, maximum_uncertainty_um: u64) -> Result<(), SpatialReason> {
        if self.source.id == self.target.id {
            return Err(SpatialReason::SameFrame);
        }
        self.source.compatible(&self.target)?;
        self.rotation.validate()?;
        if QuaternionQ30::quarter_turn_z(self.quarter_turns_z) != Some(self.rotation) {
            return Err(SpatialReason::InvalidQuaternion);
        }
        self.validity.validate()?;
        if self.uncertainty.translation_um > maximum_uncertainty_um {
            return Err(SpatialReason::ExcessiveUncertainty);
        }
        if self.calibration_identity == [0; 32] || self.provenance_identity == [0; 32] {
            return Err(SpatialReason::CalibrationMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StampedPoint3 {
    pub frame_id: String,
    pub xyz_um: [i64; 3],
    pub clock: String,
    pub tick: u64,
    pub uncertainty_um: u64,
    pub provenance_identity: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Twist3 {
    pub frame_id: String,
    pub linear_um_per_tick: [i64; 3],
    pub angular_q30_per_tick: [i32; 3],
    pub clock: String,
    pub tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockConversion {
    pub source_clock: String,
    pub target_clock: String,
    pub source_tick: u64,
    pub target_tick: u64,
    pub uncertainty_ticks: u64,
    pub valid_until_tick: u64,
}

pub fn convert_tick(
    clock: &str,
    tick: u64,
    target_clock: &str,
    conversion: Option<ClockConversion>,
) -> Result<(u64, u64), SpatialReason> {
    if clock == target_clock {
        return Ok((tick, 0));
    }
    let conversion = conversion.ok_or(SpatialReason::MissingClockConversion)?;
    if conversion.source_clock != clock
        || conversion.target_clock != target_clock
        || tick != conversion.source_tick
        || tick > conversion.valid_until_tick
    {
        return Err(SpatialReason::ClockMismatch);
    }
    Ok((conversion.target_tick, conversion.uncertainty_ticks))
}

fn rotate_quarter(point: [i64; 3], turns: u8) -> Result<[i64; 3], SpatialReason> {
    let [x, y, z] = point;
    Ok(match turns % 4 {
        0 => [x, y, z],
        1 => [y.checked_neg().ok_or(SpatialReason::NumericOverflow)?, x, z],
        2 => [
            x.checked_neg().ok_or(SpatialReason::NumericOverflow)?,
            y.checked_neg().ok_or(SpatialReason::NumericOverflow)?,
            z,
        ],
        3 => [y, x.checked_neg().ok_or(SpatialReason::NumericOverflow)?, z],
        _ => unreachable!(),
    })
}

fn add3(left: [i64; 3], right: [i64; 3]) -> Result<[i64; 3], SpatialReason> {
    Ok([
        left[0]
            .checked_add(right[0])
            .ok_or(SpatialReason::NumericOverflow)?,
        left[1]
            .checked_add(right[1])
            .ok_or(SpatialReason::NumericOverflow)?,
        left[2]
            .checked_add(right[2])
            .ok_or(SpatialReason::NumericOverflow)?,
    ])
}

pub fn apply_transform(
    transform: &Transform3,
    point: &StampedPoint3,
    profile: NumericProfile,
    maximum_uncertainty_um: u64,
) -> Result<StampedPoint3, SpatialReason> {
    profile.validate()?;
    transform.validate(maximum_uncertainty_um)?;
    if point.frame_id != transform.source.id {
        return Err(SpatialReason::WrongFrame);
    }
    if point.clock != transform.validity.clock || point.tick != transform.validity.stamp_tick {
        return Err(SpatialReason::ClockMismatch);
    }
    let xyz_um = add3(
        rotate_quarter(point.xyz_um, transform.quarter_turns_z)?,
        transform.translation_um,
    )?;
    let uncertainty_um = point
        .uncertainty_um
        .checked_add(transform.uncertainty.translation_um)
        .ok_or(SpatialReason::NumericOverflow)?;
    if uncertainty_um > maximum_uncertainty_um {
        return Err(SpatialReason::ExcessiveUncertainty);
    }
    Ok(StampedPoint3 {
        frame_id: transform.target.id.clone(),
        xyz_um,
        clock: point.clock.clone(),
        tick: point.tick,
        uncertainty_um,
        provenance_identity: transform.provenance_identity,
    })
}

pub fn compose(
    first: &Transform3,
    second: &Transform3,
    profile: NumericProfile,
    maximum_uncertainty_um: u64,
) -> Result<Transform3, SpatialReason> {
    profile.validate()?;
    first.validate(maximum_uncertainty_um)?;
    second.validate(maximum_uncertainty_um)?;
    if first.target != second.source {
        return Err(SpatialReason::WrongFrame);
    }
    if first.validity.clock != second.validity.clock
        || first.validity.stamp_tick != second.validity.stamp_tick
    {
        return Err(SpatialReason::ClockMismatch);
    }
    if first.calibration_identity != second.calibration_identity {
        return Err(SpatialReason::CalibrationMismatch);
    }
    let translation_um = add3(
        rotate_quarter(first.translation_um, second.quarter_turns_z)?,
        second.translation_um,
    )?;
    let uncertainty_um = first
        .uncertainty
        .translation_um
        .checked_add(second.uncertainty.translation_um)
        .ok_or(SpatialReason::NumericOverflow)?;
    if uncertainty_um > maximum_uncertainty_um {
        return Err(SpatialReason::ExcessiveUncertainty);
    }
    Ok(Transform3 {
        source: first.source.clone(),
        target: second.target.clone(),
        translation_um,
        rotation: QuaternionQ30::quarter_turn_z(
            (first.quarter_turns_z + second.quarter_turns_z) % 4,
        )
        .expect("sum modulo four is a supported quarter turn"),
        quarter_turns_z: (first.quarter_turns_z + second.quarter_turns_z) % 4,
        validity: Validity {
            clock: first.validity.clock.clone(),
            stamp_tick: first.validity.stamp_tick,
            valid_from_tick: first
                .validity
                .valid_from_tick
                .max(second.validity.valid_from_tick),
            valid_until_tick: first
                .validity
                .valid_until_tick
                .min(second.validity.valid_until_tick),
        },
        uncertainty: Uncertainty {
            translation_um: uncertainty_um,
            ..Uncertainty::EXACT
        },
        calibration_identity: first.calibration_identity,
        provenance_identity: first.provenance_identity,
    })
}

pub fn invert(
    transform: &Transform3,
    profile: NumericProfile,
    maximum_uncertainty_um: u64,
) -> Result<Transform3, SpatialReason> {
    profile.validate()?;
    transform.validate(maximum_uncertainty_um)?;
    let turns = (4 - transform.quarter_turns_z) % 4;
    let negated = [
        transform.translation_um[0]
            .checked_neg()
            .ok_or(SpatialReason::SingularTransform)?,
        transform.translation_um[1]
            .checked_neg()
            .ok_or(SpatialReason::SingularTransform)?,
        transform.translation_um[2]
            .checked_neg()
            .ok_or(SpatialReason::SingularTransform)?,
    ];
    Ok(Transform3 {
        source: transform.target.clone(),
        target: transform.source.clone(),
        translation_um: rotate_quarter(negated, turns)?,
        rotation: QuaternionQ30::quarter_turn_z(turns)
            .expect("inverse modulo four is a supported quarter turn"),
        quarter_turns_z: turns,
        validity: transform.validity.clone(),
        uncertainty: transform.uncertainty,
        calibration_identity: transform.calibration_identity,
        provenance_identity: transform.provenance_identity,
    })
}

pub fn interpolate(
    before: &Transform3,
    after: &Transform3,
    tick: u64,
    maximum_window_ticks: u64,
    maximum_uncertainty_um: u64,
) -> Result<Transform3, SpatialReason> {
    before.validate(maximum_uncertainty_um)?;
    after.validate(maximum_uncertainty_um)?;
    if before.source != after.source || before.target != after.target {
        return Err(SpatialReason::WrongFrame);
    }
    if before.validity.clock != after.validity.clock {
        return Err(SpatialReason::ClockMismatch);
    }
    let start = before.validity.stamp_tick;
    let end = after.validity.stamp_tick;
    let width = end
        .checked_sub(start)
        .ok_or(SpatialReason::InterpolationBoundary)?;
    if width == 0 || width > maximum_window_ticks || tick < start || tick > end {
        return Err(SpatialReason::InterpolationBoundary);
    }
    if before.quarter_turns_z != after.quarter_turns_z {
        return Err(SpatialReason::InvalidQuaternion);
    }
    let offset = tick - start;
    let mut translation_um = [0_i64; 3];
    for (index, value) in translation_um.iter_mut().enumerate() {
        let delta =
            i128::from(after.translation_um[index]) - i128::from(before.translation_um[index]);
        let interpolated = i128::from(before.translation_um[index])
            .checked_add(
                delta
                    .checked_mul(i128::from(offset))
                    .ok_or(SpatialReason::NumericOverflow)?
                    / i128::from(width),
            )
            .ok_or(SpatialReason::NumericOverflow)?;
        *value = i64::try_from(interpolated).map_err(|_| SpatialReason::NumericOverflow)?;
    }
    let uncertainty_um = before
        .uncertainty
        .translation_um
        .max(after.uncertainty.translation_um);
    if uncertainty_um > maximum_uncertainty_um {
        return Err(SpatialReason::ExcessiveUncertainty);
    }
    Ok(Transform3 {
        source: before.source.clone(),
        target: before.target.clone(),
        translation_um,
        rotation: before.rotation,
        quarter_turns_z: before.quarter_turns_z,
        validity: Validity {
            clock: before.validity.clock.clone(),
            stamp_tick: tick,
            valid_from_tick: start,
            valid_until_tick: end,
        },
        uncertainty: Uncertainty {
            translation_um: uncertainty_um,
            ..Uncertainty::EXACT
        },
        calibration_identity: before.calibration_identity,
        provenance_identity: before.provenance_identity,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinholeCalibration {
    pub frame_id: String,
    pub calibration_identity: [u8; 32],
    pub fx_millipixel: i64,
    pub fy_millipixel: i64,
    pub cx_millipixel: i64,
    pub cy_millipixel: i64,
    pub width_pixels: u32,
    pub height_pixels: u32,
    pub valid_until_tick: u64,
}

impl PinholeCalibration {
    fn validate(&self, tick: u64) -> Result<(), SpatialReason> {
        if self.frame_id.is_empty()
            || self.calibration_identity == [0; 32]
            || self.fx_millipixel <= 0
            || self.fy_millipixel <= 0
            || self.width_pixels == 0
            || self.height_pixels == 0
        {
            return Err(SpatialReason::InvalidCalibration);
        }
        if tick > self.valid_until_tick {
            return Err(SpatialReason::StaleTransform);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PixelPoint {
    pub frame_id: String,
    pub x_millipixel: i64,
    pub y_millipixel: i64,
    pub depth_um: i64,
    pub clock: String,
    pub tick: u64,
    pub calibration_identity: [u8; 32],
}

pub fn project(
    point: &StampedPoint3,
    calibration: &PinholeCalibration,
) -> Result<PixelPoint, SpatialReason> {
    calibration.validate(point.tick)?;
    if point.frame_id != calibration.frame_id {
        return Err(SpatialReason::WrongFrame);
    }
    let [x, y, z] = point.xyz_um;
    if z <= 0 {
        return Err(SpatialReason::BehindCamera);
    }
    let x_millipixel = calibration
        .cx_millipixel
        .checked_add(
            calibration
                .fx_millipixel
                .checked_mul(x)
                .ok_or(SpatialReason::NumericOverflow)?
                / z,
        )
        .ok_or(SpatialReason::NumericOverflow)?;
    let y_millipixel = calibration
        .cy_millipixel
        .checked_add(
            calibration
                .fy_millipixel
                .checked_mul(y)
                .ok_or(SpatialReason::NumericOverflow)?
                / z,
        )
        .ok_or(SpatialReason::NumericOverflow)?;
    if x_millipixel < 0
        || y_millipixel < 0
        || x_millipixel >= i64::from(calibration.width_pixels) * 1000
        || y_millipixel >= i64::from(calibration.height_pixels) * 1000
    {
        return Err(SpatialReason::BehindCamera);
    }
    Ok(PixelPoint {
        frame_id: point.frame_id.clone(),
        x_millipixel,
        y_millipixel,
        depth_um: z,
        clock: point.clock.clone(),
        tick: point.tick,
        calibration_identity: calibration.calibration_identity,
    })
}

pub fn unproject(
    pixel: &PixelPoint,
    calibration: &PinholeCalibration,
) -> Result<StampedPoint3, SpatialReason> {
    calibration.validate(pixel.tick)?;
    if pixel.frame_id != calibration.frame_id {
        return Err(SpatialReason::WrongFrame);
    }
    if pixel.calibration_identity != calibration.calibration_identity {
        return Err(SpatialReason::CalibrationMismatch);
    }
    if pixel.depth_um <= 0 {
        return Err(SpatialReason::SingularTransform);
    }
    let x = (pixel.x_millipixel - calibration.cx_millipixel)
        .checked_mul(pixel.depth_um)
        .ok_or(SpatialReason::NumericOverflow)?
        / calibration.fx_millipixel;
    let y = (pixel.y_millipixel - calibration.cy_millipixel)
        .checked_mul(pixel.depth_um)
        .ok_or(SpatialReason::NumericOverflow)?
        / calibration.fy_millipixel;
    Ok(StampedPoint3 {
        frame_id: pixel.frame_id.clone(),
        xyz_um: [x, y, pixel.depth_um],
        clock: pixel.clock.clone(),
        tick: pixel.tick,
        uncertainty_um: 0,
        provenance_identity: PROVENANCE_IDENTITY,
    })
}

pub fn validate_acyclic_frames(edges: &[(&str, &str)]) -> Result<(), SpatialReason> {
    if edges.len() > MAXIMUM_TRANSFORM_EDGES {
        return Err(SpatialReason::HistoryOverflow);
    }
    for (index, (source, target)) in edges.iter().enumerate() {
        if source == target {
            return Err(SpatialReason::FrameCycle);
        }
        let mut current = *target;
        for _ in 0..=edges.len() {
            if current == *source {
                return Err(SpatialReason::FrameCycle);
            }
            let Some((_, next)) = edges
                .iter()
                .take(index + 1)
                .find(|(candidate, _)| *candidate == current)
            else {
                break;
            };
            current = next;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform(source: &str, target: &str, translation_um: [i64; 3], tick: u64) -> Transform3 {
        Transform3 {
            source: FrameIdentity::new(source).unwrap(),
            target: FrameIdentity::new(target).unwrap(),
            translation_um,
            rotation: QuaternionQ30::IDENTITY,
            quarter_turns_z: 0,
            validity: Validity {
                clock: "clock/fixture".to_owned(),
                stamp_tick: tick,
                valid_from_tick: 0,
                valid_until_tick: 20,
            },
            uncertainty: Uncertainty::EXACT,
            calibration_identity: CALIBRATION_IDENTITY,
            provenance_identity: PROVENANCE_IDENTITY,
        }
    }

    fn calibration() -> PinholeCalibration {
        PinholeCalibration {
            frame_id: "camera".to_owned(),
            calibration_identity: CALIBRATION_IDENTITY,
            fx_millipixel: 100_000,
            fy_millipixel: 100_000,
            cx_millipixel: 320_000,
            cy_millipixel: 240_000,
            width_pixels: 640,
            height_pixels: 480,
            valid_until_tick: 20,
        }
    }

    #[test]
    fn compose_invert_interpolate_and_apply_are_exact() {
        let first = transform("sensor", "body", [10, 20, 30], 10);
        let second = transform("body", "map", [100, 200, 300], 10);
        let composed = compose(&first, &second, NumericProfile::FIRST_PROOF, 10).unwrap();
        assert_eq!(composed.translation_um, [110, 220, 330]);
        let inverse = invert(&composed, NumericProfile::FIRST_PROOF, 10).unwrap();
        assert_eq!(inverse.translation_um, [-110, -220, -330]);
        let point = StampedPoint3 {
            frame_id: "sensor".to_owned(),
            xyz_um: [1, 2, 3],
            clock: "clock/fixture".to_owned(),
            tick: 10,
            uncertainty_um: 0,
            provenance_identity: PROVENANCE_IDENTITY,
        };
        assert_eq!(
            apply_transform(&composed, &point, NumericProfile::FIRST_PROOF, 10)
                .unwrap()
                .xyz_um,
            [111, 222, 333]
        );
        let before = transform("sensor", "map", [0, 0, 0], 10);
        let after = transform("sensor", "map", [20, 40, 60], 12);
        assert_eq!(
            interpolate(&before, &after, 11, 4, 10)
                .unwrap()
                .translation_um,
            [10, 20, 30]
        );
    }

    #[test]
    fn projection_round_trip_is_exact_for_the_fixed_profile() {
        let point = StampedPoint3 {
            frame_id: "camera".to_owned(),
            xyz_um: [1000, 500, 10_000],
            clock: "clock/fixture".to_owned(),
            tick: 10,
            uncertainty_um: 0,
            provenance_identity: PROVENANCE_IDENTITY,
        };
        let pixel = project(&point, &calibration()).unwrap();
        assert_eq!((pixel.x_millipixel, pixel.y_millipixel), (330_000, 245_000));
        assert_eq!(unproject(&pixel, &calibration()).unwrap(), point);
    }

    #[test]
    fn cycles_clocks_uncertainty_singularity_and_calibration_fail_closed() {
        assert_eq!(
            validate_acyclic_frames(&[("a", "b"), ("b", "a")]),
            Err(SpatialReason::FrameCycle)
        );
        assert_eq!(
            convert_tick("clock/a", 10, "clock/b", None),
            Err(SpatialReason::MissingClockConversion)
        );
        let mut uncertain = transform("a", "b", [0; 3], 10);
        uncertain.uncertainty.translation_um = 11;
        assert_eq!(
            uncertain.validate(10),
            Err(SpatialReason::ExcessiveUncertainty)
        );
        let behind = StampedPoint3 {
            frame_id: "camera".to_owned(),
            xyz_um: [0, 0, 0],
            clock: "clock/fixture".to_owned(),
            tick: 10,
            uncertainty_um: 0,
            provenance_identity: PROVENANCE_IDENTITY,
        };
        assert_eq!(
            project(&behind, &calibration()),
            Err(SpatialReason::BehindCamera)
        );
        let mut wrong = calibration();
        wrong.calibration_identity = [0; 32];
        assert_eq!(
            project(&behind, &wrong),
            Err(SpatialReason::InvalidCalibration)
        );
    }
}
