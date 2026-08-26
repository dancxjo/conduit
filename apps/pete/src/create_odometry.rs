//! Deterministic start-local odometry over Create distance/angle deltas.

use conduit_core::{BootId, HostId, OdometryObservation, OfferGeneration, MAXIMUM_ODOMETRY_MM};

const SCALE: i64 = 1_000_000;
const DEGREES_PER_TURN: i64 = 360;
const PI_MICRORADIANS: i64 = conduit_core::PI_MICRORADIANS as i64;
const HALF_PI_MICRORADIANS: i64 = conduit_core::HALF_PI_MICRORADIANS as i64;
const TURN_NUMERATOR: i64 = DEGREES_PER_TURN * PI_MICRORADIANS;
const HALF_TURN_NUMERATOR: i64 = TURN_NUMERATOR / 2;
const POSITION_LIMIT_SCALED: i64 = conduit_core::MAXIMUM_ODOMETRY_MM as i64 * SCALE;
const CORDIC_GAIN_INVERSE: i64 = 607_253;
const CORDIC_ATAN_MICRORADIANS: [i64; 20] = [
    785_398, 463_648, 244_979, 124_355, 62_419, 31_240, 15_624, 7_812, 3_906, 1_953, 977, 488, 244,
    122, 61, 31, 15, 8, 4, 2,
];

pub const CREATE_ODOMETRY_RESET_AUTHORITY: &str = "pete.authority/create1-reset-odometry@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOdometrySample {
    pub value: OdometryObservation,
    pub frame_generation: u32,
    pub sample_generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateOdometryError {
    PositionOverflow,
    SampleGenerationExhausted,
    PortableValueOutsideContract,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOdometryResetRequest<'a> {
    pub request_id: &'a str,
    pub expected_frame_generation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateOdometryResetAuthority<'a> {
    pub grant_id: &'a str,
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub valid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOdometryResetBinding<'a> {
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateOdometryResetSign {
    pub request_id: String,
    pub authority_grant_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: String,
    pub prior_frame_generation: u32,
    pub current_frame_generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateOdometryResetRefusal {
    MissingRequestIdentity,
    MissingAuthority,
    WrongAuthority,
    AuthorityExpired,
    HostMismatch,
    BootMismatch,
    OfferGenerationMismatch,
    ImplementationMismatch,
    StaleFrameGeneration,
    FrameGenerationExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOdometryAccumulator {
    frame_generation: u32,
    sample_generation: u32,
    forward_scaled: i64,
    lateral_scaled: i64,
    yaw_numerator: i64,
}

impl CreateOdometryAccumulator {
    pub const fn new() -> Self {
        Self {
            frame_generation: 1,
            sample_generation: 0,
            forward_scaled: 0,
            lateral_scaled: 0,
            yaw_numerator: 0,
        }
    }

    pub const fn frame_generation(self) -> u32 {
        self.frame_generation
    }

    pub const fn sample_generation(self) -> u32 {
        self.sample_generation
    }

    pub fn current(self) -> Result<CreateOdometrySample, CreateOdometryError> {
        sample(
            self.forward_scaled,
            self.lateral_scaled,
            self.yaw_numerator,
            self.frame_generation,
            self.sample_generation,
        )
    }

    pub fn integrate(
        &mut self,
        distance_delta_mm: i16,
        angle_delta_degrees: i16,
    ) -> Result<CreateOdometrySample, CreateOdometryError> {
        let next_sample = self
            .sample_generation
            .checked_add(1)
            .ok_or(CreateOdometryError::SampleGenerationExhausted)?;
        let delta_yaw_numerator = i64::from(angle_delta_degrees) * PI_MICRORADIANS;
        let midpoint_numerator = normalize_yaw_numerator(
            self.yaw_numerator
                .checked_add(delta_yaw_numerator / 2)
                .ok_or(CreateOdometryError::PositionOverflow)?,
        );
        let midpoint_microradians = rounded_div(midpoint_numerator, 180);
        let (sine, cosine) = fixed_sin_cos(midpoint_microradians);
        let distance = i64::from(distance_delta_mm);
        let forward_delta = distance
            .checked_mul(cosine)
            .ok_or(CreateOdometryError::PositionOverflow)?;
        let lateral_delta = distance
            .checked_mul(sine)
            .ok_or(CreateOdometryError::PositionOverflow)?;
        let next_forward = self
            .forward_scaled
            .checked_add(forward_delta)
            .filter(|value| value.unsigned_abs() <= POSITION_LIMIT_SCALED as u64)
            .ok_or(CreateOdometryError::PositionOverflow)?;
        let next_lateral = self
            .lateral_scaled
            .checked_add(lateral_delta)
            .filter(|value| value.unsigned_abs() <= POSITION_LIMIT_SCALED as u64)
            .ok_or(CreateOdometryError::PositionOverflow)?;
        let next_yaw = normalize_yaw_numerator(
            self.yaw_numerator
                .checked_add(delta_yaw_numerator)
                .ok_or(CreateOdometryError::PositionOverflow)?,
        );
        let next = sample(
            next_forward,
            next_lateral,
            next_yaw,
            self.frame_generation,
            next_sample,
        )?;
        self.forward_scaled = next_forward;
        self.lateral_scaled = next_lateral;
        self.yaw_numerator = next_yaw;
        self.sample_generation = next_sample;
        Ok(next)
    }

    pub fn reset(
        &mut self,
        request: CreateOdometryResetRequest<'_>,
        authority: Option<CreateOdometryResetAuthority<'_>>,
        binding: CreateOdometryResetBinding<'_>,
        now_tick: u64,
    ) -> Result<CreateOdometryResetSign, CreateOdometryResetRefusal> {
        if request.request_id.is_empty() {
            return Err(CreateOdometryResetRefusal::MissingRequestIdentity);
        }
        let authority = authority.ok_or(CreateOdometryResetRefusal::MissingAuthority)?;
        if authority.grant_id != CREATE_ODOMETRY_RESET_AUTHORITY {
            return Err(CreateOdometryResetRefusal::WrongAuthority);
        }
        if authority.valid_until_tick <= now_tick {
            return Err(CreateOdometryResetRefusal::AuthorityExpired);
        }
        if authority.host_id != binding.host_id {
            return Err(CreateOdometryResetRefusal::HostMismatch);
        }
        if authority.boot_id != binding.boot_id {
            return Err(CreateOdometryResetRefusal::BootMismatch);
        }
        if authority.offer_generation != binding.offer_generation {
            return Err(CreateOdometryResetRefusal::OfferGenerationMismatch);
        }
        if authority.implementation_id != binding.implementation_id {
            return Err(CreateOdometryResetRefusal::ImplementationMismatch);
        }
        if request.expected_frame_generation != self.frame_generation {
            return Err(CreateOdometryResetRefusal::StaleFrameGeneration);
        }
        let next_generation = self
            .frame_generation
            .checked_add(1)
            .ok_or(CreateOdometryResetRefusal::FrameGenerationExhausted)?;
        let prior = self.frame_generation;
        self.frame_generation = next_generation;
        self.sample_generation = 0;
        self.forward_scaled = 0;
        self.lateral_scaled = 0;
        self.yaw_numerator = 0;
        Ok(CreateOdometryResetSign {
            request_id: request.request_id.to_string(),
            authority_grant_id: authority.grant_id.to_string(),
            host_id: binding.host_id.clone(),
            boot_id: binding.boot_id.clone(),
            offer_generation: binding.offer_generation,
            implementation_id: binding.implementation_id.to_string(),
            prior_frame_generation: prior,
            current_frame_generation: next_generation,
        })
    }
}

impl Default for CreateOdometryAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

fn sample(
    forward_scaled: i64,
    lateral_scaled: i64,
    yaw_numerator: i64,
    frame_generation: u32,
    sample_generation: u32,
) -> Result<CreateOdometrySample, CreateOdometryError> {
    let forward = i32::try_from(rounded_div(forward_scaled, SCALE))
        .map_err(|_| CreateOdometryError::PortableValueOutsideContract)?;
    let lateral = i32::try_from(rounded_div(lateral_scaled, SCALE))
        .map_err(|_| CreateOdometryError::PortableValueOutsideContract)?;
    let yaw = i32::try_from(rounded_div(normalize_yaw_numerator(yaw_numerator), 180))
        .map_err(|_| CreateOdometryError::PortableValueOutsideContract)?;
    if forward.unsigned_abs() > MAXIMUM_ODOMETRY_MM as u32
        || lateral.unsigned_abs() > MAXIMUM_ODOMETRY_MM as u32
    {
        return Err(CreateOdometryError::PortableValueOutsideContract);
    }
    let value = OdometryObservation::new(forward, lateral, yaw)
        .map_err(|_| CreateOdometryError::PortableValueOutsideContract)?;
    Ok(CreateOdometrySample {
        value,
        frame_generation,
        sample_generation,
    })
}

fn normalize_yaw_numerator(value: i64) -> i64 {
    let mut normalized = value.rem_euclid(TURN_NUMERATOR);
    if normalized > HALF_TURN_NUMERATOR {
        normalized -= TURN_NUMERATOR;
    }
    normalized
}

fn rounded_div(numerator: i64, denominator: i64) -> i64 {
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        (numerator - denominator / 2) / denominator
    }
}

fn fixed_sin_cos(angle_microradians: i64) -> (i64, i64) {
    let mut angle = angle_microradians;
    let mut cosine_sign = 1_i64;
    if angle > HALF_PI_MICRORADIANS {
        angle = PI_MICRORADIANS - angle;
        cosine_sign = -1;
    } else if angle < -HALF_PI_MICRORADIANS {
        angle = -PI_MICRORADIANS - angle;
        cosine_sign = -1;
    }
    let mut x = CORDIC_GAIN_INVERSE;
    let mut y = 0_i64;
    let mut z = angle;
    for (shift, arctangent) in CORDIC_ATAN_MICRORADIANS.into_iter().enumerate() {
        let prior_x = x;
        if z >= 0 {
            x -= y >> shift;
            y += prior_x >> shift;
            z -= arctangent;
        } else {
            x += y >> shift;
            y -= prior_x >> shift;
            z += arctangent;
        }
    }
    (y, x * cosine_sign)
}

#[cfg(test)]
#[path = "create_odometry_tests.rs"]
mod tests;
