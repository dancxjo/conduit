use crate::RawImuSample;

pub const PI_MICRORADIANS: i32 = 3_141_593;
pub const HALF_PI_MICRORADIANS: i32 = 1_570_797;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GravityCalibration {
    pub generation: u64,
    pub reference_x_mm_s2: i16,
    pub reference_y_mm_s2: i16,
    pub reference_z_mm_s2: i16,
    pub captured_at_tick: u64,
}

impl GravityCalibration {
    pub fn capture(generation: u64, sample: RawImuSample) -> Result<Self, DerivationFailure> {
        if generation == 0 {
            return Err(DerivationFailure::InvalidCalibrationGeneration);
        }
        if vector_magnitude(
            i32::from(sample.accel_x_mm_s2),
            i32::from(sample.accel_y_mm_s2),
            i32::from(sample.accel_z_mm_s2),
        ) < 1_000
        {
            return Err(DerivationFailure::InvalidGravityReference);
        }
        Ok(Self {
            generation,
            reference_x_mm_s2: sample.accel_x_mm_s2,
            reference_y_mm_s2: sample.accel_y_mm_s2,
            reference_z_mm_s2: sample.accel_z_mm_s2,
            captured_at_tick: sample.observed_at_tick,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImuThresholds {
    pub tilt_stop_microradians: u32,
    pub impact_stop_mm_s2: u16,
    pub maximum_sample_age_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivedImuObservation {
    pub roll_microradians: i32,
    pub pitch_microradians: i32,
    pub yaw_microradians: i32,
    pub acceleration_magnitude_mm_s2: u16,
    pub tilt_magnitude_microradians: u32,
    pub impact_score_mm_s2: u16,
    pub tilt_active: bool,
    pub impact_active: bool,
    pub observed_at_tick: u64,
    pub calibration_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivationFailure {
    InvalidCalibrationGeneration,
    InvalidGravityReference,
    CalibrationAfterSample,
    ClockRegressed,
    SampleStale,
    InvalidThresholds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImuDeriver {
    calibration: GravityCalibration,
    yaw_microradians: i32,
    previous_tick: Option<u64>,
    previous_acceleration_magnitude_mm_s2: Option<u16>,
}

impl ImuDeriver {
    pub const fn new(calibration: GravityCalibration) -> Self {
        Self {
            calibration,
            yaw_microradians: 0,
            previous_tick: None,
            previous_acceleration_magnitude_mm_s2: None,
        }
    }

    pub fn derive(
        &mut self,
        sample: RawImuSample,
        now_tick: u64,
        thresholds: ImuThresholds,
    ) -> Result<DerivedImuObservation, DerivationFailure> {
        validate_thresholds(thresholds)?;
        if sample.observed_at_tick < self.calibration.captured_at_tick {
            return Err(DerivationFailure::CalibrationAfterSample);
        }
        if now_tick < sample.observed_at_tick
            || self
                .previous_tick
                .is_some_and(|previous| sample.observed_at_tick < previous)
        {
            return Err(DerivationFailure::ClockRegressed);
        }
        if now_tick - sample.observed_at_tick > thresholds.maximum_sample_age_ticks {
            return Err(DerivationFailure::SampleStale);
        }

        let elapsed = self
            .previous_tick
            .map_or(0, |previous| sample.observed_at_tick - previous);
        let yaw_delta = i64::from(sample.gyro_z_milliradians_s).saturating_mul(elapsed as i64);
        self.yaw_microradians =
            normalize_angle(i64::from(self.yaw_microradians).saturating_add(yaw_delta) as i32);

        let acceleration = vector_magnitude(
            i32::from(sample.accel_x_mm_s2),
            i32::from(sample.accel_y_mm_s2),
            i32::from(sample.accel_z_mm_s2),
        );
        let impact = self
            .previous_acceleration_magnitude_mm_s2
            .map_or(0, |previous| acceleration.abs_diff(previous));
        let (roll, pitch, tilt) = calibrated_tilt(sample, self.calibration);
        self.previous_tick = Some(sample.observed_at_tick);
        self.previous_acceleration_magnitude_mm_s2 = Some(acceleration);

        Ok(DerivedImuObservation {
            roll_microradians: roll,
            pitch_microradians: pitch,
            yaw_microradians: self.yaw_microradians,
            acceleration_magnitude_mm_s2: acceleration,
            tilt_magnitude_microradians: tilt,
            impact_score_mm_s2: impact,
            tilt_active: tilt > thresholds.tilt_stop_microradians,
            impact_active: impact > thresholds.impact_stop_mm_s2,
            observed_at_tick: sample.observed_at_tick,
            calibration_generation: self.calibration.generation,
        })
    }
}

fn validate_thresholds(thresholds: ImuThresholds) -> Result<(), DerivationFailure> {
    if thresholds.tilt_stop_microradians == 0
        || thresholds.tilt_stop_microradians > PI_MICRORADIANS as u32
        || thresholds.impact_stop_mm_s2 == 0
        || thresholds.maximum_sample_age_ticks == 0
    {
        Err(DerivationFailure::InvalidThresholds)
    } else {
        Ok(())
    }
}

fn calibrated_tilt(sample: RawImuSample, calibration: GravityCalibration) -> (i32, i32, u32) {
    let rx = i64::from(calibration.reference_x_mm_s2);
    let ry = i64::from(calibration.reference_y_mm_s2);
    let rz = i64::from(calibration.reference_z_mm_s2);
    let cx = i64::from(sample.accel_x_mm_s2);
    let cy = i64::from(sample.accel_y_mm_s2);
    let cz = i64::from(sample.accel_z_mm_s2);
    let cross_x = ry.saturating_mul(cz).saturating_sub(rz.saturating_mul(cy));
    let cross_y = rz.saturating_mul(cx).saturating_sub(rx.saturating_mul(cz));
    let cross_z = rx.saturating_mul(cy).saturating_sub(ry.saturating_mul(cx));
    let dot = rx
        .saturating_mul(cx)
        .saturating_add(ry.saturating_mul(cy))
        .saturating_add(rz.saturating_mul(cz));
    let denominator = abs_i64(dot).max(1);
    let roll = clamp_angle(
        cross_x.saturating_mul(1_000_000) / denominator,
        HALF_PI_MICRORADIANS,
    );
    let pitch = clamp_angle(
        cross_y.saturating_mul(1_000_000) / denominator,
        HALF_PI_MICRORADIANS,
    );
    let cross_magnitude = vector_magnitude_u64(cross_x, cross_y, cross_z);
    let tilt = cross_magnitude
        .saturating_mul(1_000_000)
        .checked_div(denominator as u64)
        .unwrap_or(u64::MAX)
        .min(PI_MICRORADIANS as u64) as u32;
    (roll, pitch, tilt)
}

fn normalize_angle(value: i32) -> i32 {
    let turn = PI_MICRORADIANS * 2;
    let mut normalized = value % turn;
    if normalized > PI_MICRORADIANS {
        normalized -= turn;
    }
    if normalized < -PI_MICRORADIANS {
        normalized += turn;
    }
    normalized
}

fn clamp_angle(value: i64, maximum: i32) -> i32 {
    value.clamp(-i64::from(maximum), i64::from(maximum)) as i32
}

fn vector_magnitude(x: i32, y: i32, z: i32) -> u16 {
    vector_magnitude_u64(i64::from(x), i64::from(y), i64::from(z)).min(u64::from(u16::MAX)) as u16
}

fn vector_magnitude_u64(x: i64, y: i64, z: i64) -> u64 {
    int_sqrt(
        x.saturating_mul(x)
            .saturating_add(y.saturating_mul(y))
            .saturating_add(z.saturating_mul(z)) as u64,
    )
}

fn int_sqrt(value: u64) -> u64 {
    let mut result = 0_u64;
    let mut bit = 1_u64 << 62;
    while bit > value {
        bit >>= 2;
    }
    let mut remainder = value;
    while bit != 0 {
        if remainder >= result + bit {
            remainder -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    result
}

fn abs_i64(value: i64) -> i64 {
    if value == i64::MIN {
        i64::MAX
    } else {
        value.abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THRESHOLDS: ImuThresholds = ImuThresholds {
        tilt_stop_microradians: 650_000,
        impact_stop_mm_s2: 4_000,
        maximum_sample_age_ticks: 100,
    };

    #[test]
    fn mounting_calibration_projects_portable_orientation_and_hazards() {
        let level = RawImuSample {
            accel_x_mm_s2: 9_807,
            accel_y_mm_s2: 0,
            accel_z_mm_s2: 0,
            ..RawImuSample::stationary(10)
        };
        let calibration = GravityCalibration::capture(1, level).unwrap();
        let mut deriver = ImuDeriver::new(calibration);
        let level_observation = deriver.derive(level, 10, THRESHOLDS).unwrap();
        assert_eq!(
            (
                level_observation.roll_microradians,
                level_observation.pitch_microradians
            ),
            (0, 0)
        );
        assert!(!level_observation.tilt_active);

        let tilted = RawImuSample {
            observed_at_tick: 20,
            accel_x_mm_s2: 7_000,
            accel_y_mm_s2: 14_000,
            accel_z_mm_s2: 0,
            ..level
        };
        let tilted_observation = deriver.derive(tilted, 20, THRESHOLDS).unwrap();
        assert!(tilted_observation.tilt_active);
        assert!(tilted_observation.impact_active);
    }

    #[test]
    fn stale_regressed_and_invalid_calibration_refuse_distinctly() {
        assert_eq!(
            GravityCalibration::capture(0, RawImuSample::stationary(1)),
            Err(DerivationFailure::InvalidCalibrationGeneration)
        );
        let zero = RawImuSample {
            accel_z_mm_s2: 0,
            ..RawImuSample::stationary(1)
        };
        assert_eq!(
            GravityCalibration::capture(1, zero),
            Err(DerivationFailure::InvalidGravityReference)
        );

        let sample = RawImuSample::stationary(10);
        let mut deriver = ImuDeriver::new(GravityCalibration::capture(1, sample).unwrap());
        assert_eq!(
            deriver.derive(sample, 111, THRESHOLDS),
            Err(DerivationFailure::SampleStale)
        );
        deriver
            .derive(RawImuSample::stationary(20), 20, THRESHOLDS)
            .unwrap();
        assert_eq!(
            deriver.derive(RawImuSample::stationary(19), 20, THRESHOLDS),
            Err(DerivationFailure::ClockRegressed)
        );
    }
}
