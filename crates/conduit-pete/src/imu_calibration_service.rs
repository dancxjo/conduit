//! Exact authorized zero-orientation transition for one MPU-6050 attachment.

use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_mpu6050::{GravityCalibration, RawImuSample};

pub const IMU_CALIBRATION_AUTHORITY: &str = "pete.authority/imu-calibration@1";
pub const IMU_CALIBRATION_SERVICE: &str = "pete/mpu6050-calibration-service@1";
pub const MAXIMUM_CALIBRATION_GYRO_MILLIRADIANS_S: u16 = 50;
pub const MINIMUM_CALIBRATION_GRAVITY_MM_S2: u16 = 8_000;
pub const MAXIMUM_CALIBRATION_GRAVITY_MM_S2: u16 = 12_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImuCalibrationBinding<'a> {
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub i2c_base_id: &'a str,
    pub attachment_id: &'a str,
    pub body_frame_id: &'a str,
    pub mounting_id: &'a str,
    pub current_calibration_generation: u64,
    pub maximum_sample_age_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImuCalibrationRequest<'a> {
    pub request_id: &'a str,
    pub expected_calibration_generation: u64,
    pub deadline_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImuCalibrationAuthority<'a> {
    pub grant_id: &'a str,
    pub contract_id: &'a str,
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub attachment_id: &'a str,
    pub valid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImuCalibrationRefusal {
    MissingIdentity,
    MissingRequestIdentity,
    InvalidFreshness,
    SampleFromFuture,
    StaleSample,
    CalibrationGenerationMismatch,
    CalibrationGenerationExhausted,
    BodyNotStationary,
    InvalidGravity,
    InvalidDeadline,
    MissingAuthority,
    WrongAuthority,
    AuthorityExpired,
    OperationOutlivesAuthority,
    HostMismatch,
    BootMismatch,
    OfferGenerationMismatch,
    ImplementationMismatch,
    AttachmentMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImuCalibrationSign {
    pub request_id: String,
    pub authority_grant_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: String,
    pub i2c_base_id: String,
    pub attachment_id: String,
    pub body_frame_id: String,
    pub mounting_id: String,
    pub prior_calibration_generation: u64,
    pub calibration: GravityCalibration,
    pub sample: RawImuSample,
    pub deadline_tick: u64,
}

pub fn zero_imu_orientation(
    binding: ImuCalibrationBinding<'_>,
    request: ImuCalibrationRequest<'_>,
    authority: Option<ImuCalibrationAuthority<'_>>,
    sample: RawImuSample,
    now_tick: u64,
) -> Result<ImuCalibrationSign, ImuCalibrationRefusal> {
    validate(&binding, request, authority.as_ref(), sample, now_tick)?;
    let authority = authority.expect("validated IMU calibration authority");
    let next_generation = binding
        .current_calibration_generation
        .checked_add(1)
        .ok_or(ImuCalibrationRefusal::CalibrationGenerationExhausted)?;
    let calibration = GravityCalibration::capture(next_generation, sample)
        .map_err(|_| ImuCalibrationRefusal::InvalidGravity)?;
    Ok(ImuCalibrationSign {
        request_id: request.request_id.to_string(),
        authority_grant_id: authority.grant_id.to_string(),
        host_id: binding.host_id.clone(),
        boot_id: binding.boot_id.clone(),
        offer_generation: binding.offer_generation,
        implementation_id: binding.implementation_id.to_string(),
        i2c_base_id: binding.i2c_base_id.to_string(),
        attachment_id: binding.attachment_id.to_string(),
        body_frame_id: binding.body_frame_id.to_string(),
        mounting_id: binding.mounting_id.to_string(),
        prior_calibration_generation: binding.current_calibration_generation,
        calibration,
        sample,
        deadline_tick: request.deadline_tick,
    })
}

fn validate(
    binding: &ImuCalibrationBinding<'_>,
    request: ImuCalibrationRequest<'_>,
    authority: Option<&ImuCalibrationAuthority<'_>>,
    sample: RawImuSample,
    now_tick: u64,
) -> Result<(), ImuCalibrationRefusal> {
    if binding.implementation_id.is_empty()
        || binding.i2c_base_id.is_empty()
        || binding.attachment_id.is_empty()
        || binding.body_frame_id.is_empty()
        || binding.mounting_id.is_empty()
    {
        return Err(ImuCalibrationRefusal::MissingIdentity);
    }
    if request.request_id.is_empty() {
        return Err(ImuCalibrationRefusal::MissingRequestIdentity);
    }
    if binding.current_calibration_generation == 0 || binding.maximum_sample_age_ticks == 0 {
        return Err(ImuCalibrationRefusal::InvalidFreshness);
    }
    if sample.observed_at_tick > now_tick {
        return Err(ImuCalibrationRefusal::SampleFromFuture);
    }
    if now_tick - sample.observed_at_tick > binding.maximum_sample_age_ticks {
        return Err(ImuCalibrationRefusal::StaleSample);
    }
    if request.expected_calibration_generation != binding.current_calibration_generation {
        return Err(ImuCalibrationRefusal::CalibrationGenerationMismatch);
    }
    if binding.current_calibration_generation == u64::MAX {
        return Err(ImuCalibrationRefusal::CalibrationGenerationExhausted);
    }
    if [
        sample.gyro_x_milliradians_s,
        sample.gyro_y_milliradians_s,
        sample.gyro_z_milliradians_s,
    ]
    .into_iter()
    .any(|value| value.unsigned_abs() > MAXIMUM_CALIBRATION_GYRO_MILLIRADIANS_S)
    {
        return Err(ImuCalibrationRefusal::BodyNotStationary);
    }
    let gravity = magnitude(
        sample.accel_x_mm_s2,
        sample.accel_y_mm_s2,
        sample.accel_z_mm_s2,
    );
    if !(MINIMUM_CALIBRATION_GRAVITY_MM_S2..=MAXIMUM_CALIBRATION_GRAVITY_MM_S2).contains(&gravity) {
        return Err(ImuCalibrationRefusal::InvalidGravity);
    }
    if request.deadline_tick <= now_tick {
        return Err(ImuCalibrationRefusal::InvalidDeadline);
    }
    let authority = authority.ok_or(ImuCalibrationRefusal::MissingAuthority)?;
    if authority.grant_id.is_empty() || authority.contract_id != IMU_CALIBRATION_AUTHORITY {
        return Err(ImuCalibrationRefusal::WrongAuthority);
    }
    if authority.valid_until_tick < now_tick {
        return Err(ImuCalibrationRefusal::AuthorityExpired);
    }
    if request.deadline_tick > authority.valid_until_tick {
        return Err(ImuCalibrationRefusal::OperationOutlivesAuthority);
    }
    for (matches, refusal) in [
        (
            authority.host_id == binding.host_id,
            ImuCalibrationRefusal::HostMismatch,
        ),
        (
            authority.boot_id == binding.boot_id,
            ImuCalibrationRefusal::BootMismatch,
        ),
        (
            authority.offer_generation == binding.offer_generation,
            ImuCalibrationRefusal::OfferGenerationMismatch,
        ),
        (
            authority.implementation_id == binding.implementation_id,
            ImuCalibrationRefusal::ImplementationMismatch,
        ),
        (
            authority.attachment_id == binding.attachment_id,
            ImuCalibrationRefusal::AttachmentMismatch,
        ),
    ] {
        if !matches {
            return Err(refusal);
        }
    }
    Ok(())
}

fn magnitude(x: i16, y: i16, z: i16) -> u16 {
    let x = i64::from(x);
    let y = i64::from(y);
    let z = i64::from(z);
    let mut value = x
        .saturating_mul(x)
        .saturating_add(y.saturating_mul(y))
        .saturating_add(z.saturating_mul(z)) as u64;
    let mut result = 0_u64;
    let mut bit = 1_u64 << 62;
    while bit > value {
        bit >>= 2;
    }
    while bit != 0 {
        if value >= result + bit {
            value -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    result.min(u64::from(u16::MAX)) as u16
}

#[cfg(test)]
#[path = "imu_calibration_service_tests.rs"]
mod tests;
