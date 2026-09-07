//! Bounded canonical payloads for hysteresis profiles and decisions.

use alloc::{string::String, vec::Vec};
use conduit_core::{Quantity, TemporalInstant, TemporalScale};

use crate::{
    MeasurementHysteresis, MeasurementHysteresisProfile, MeasurementThresholdDecision,
    MeasurementThresholdState, MeasurementThresholdTransition, MeasurementWireRefusal,
};

pub const MAXIMUM_MEASUREMENT_HYSTERESIS_PROFILE_BYTES: usize = 256;
pub const MAXIMUM_MEASUREMENT_THRESHOLD_DECISION_BYTES: usize = 1_024;

pub fn encode_measurement_hysteresis_profile(
    profile: MeasurementHysteresisProfile,
) -> Result<Vec<u8>, MeasurementWireRefusal> {
    MeasurementHysteresis::new(profile.policy, profile.initial_state)
        .map_err(|_| MeasurementWireRefusal::Malformed)?;
    let mut bytes = Vec::with_capacity(2 + conduit_core::QUANTITY_ENCODED_LEN * 2);
    bytes.push(1);
    bytes.extend_from_slice(&profile.policy.lower.encode());
    bytes.extend_from_slice(&profile.policy.upper.encode());
    bytes.push(state_byte(profile.initial_state));
    Ok(bytes)
}

pub fn decode_measurement_hysteresis_profile(
    bytes: &[u8],
) -> Result<MeasurementHysteresisProfile, MeasurementWireRefusal> {
    if bytes.len() > MAXIMUM_MEASUREMENT_HYSTERESIS_PROFILE_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    let mut input = Input::new(bytes);
    if input.u8()? != 1 {
        return Err(MeasurementWireRefusal::UnsupportedVersion);
    }
    let profile = MeasurementHysteresisProfile {
        policy: crate::MeasurementThresholdPolicy {
            lower: input.quantity()?,
            upper: input.quantity()?,
        },
        initial_state: decode_state(input.u8()?)?,
    };
    if !input.finished() {
        return Err(MeasurementWireRefusal::Malformed);
    }
    encode_measurement_hysteresis_profile(profile)?;
    Ok(profile)
}

pub fn encode_measurement_threshold_decision(
    decision: &MeasurementThresholdDecision,
) -> Result<Vec<u8>, MeasurementWireRefusal> {
    decision
        .first_observed_at
        .validate()
        .map_err(|_| MeasurementWireRefusal::Malformed)?;
    decision
        .last_observed_at
        .validate()
        .map_err(|_| MeasurementWireRefusal::Malformed)?;
    if !matches!(
        decision
            .last_observed_at
            .relation_to(&decision.first_observed_at),
        Ok(conduit_core::TemporalRelation::Future { .. } | conduit_core::TemporalRelation::Present)
    ) {
        return Err(MeasurementWireRefusal::Malformed);
    }
    let mut bytes = Vec::with_capacity(128);
    bytes.push(1);
    bytes.push(state_byte(decision.state));
    bytes.push(match decision.transition {
        None => 0,
        Some(MeasurementThresholdTransition::RoseAbove) => 1,
        Some(MeasurementThresholdTransition::FellBelow) => 2,
    });
    bytes.extend_from_slice(&decision.evaluated_value.encode());
    put_instant(&mut bytes, &decision.first_observed_at)?;
    put_instant(&mut bytes, &decision.last_observed_at)?;
    if bytes.len() > MAXIMUM_MEASUREMENT_THRESHOLD_DECISION_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    Ok(bytes)
}

pub fn decode_measurement_threshold_decision(
    bytes: &[u8],
) -> Result<MeasurementThresholdDecision, MeasurementWireRefusal> {
    if bytes.len() > MAXIMUM_MEASUREMENT_THRESHOLD_DECISION_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    let mut input = Input::new(bytes);
    if input.u8()? != 1 {
        return Err(MeasurementWireRefusal::UnsupportedVersion);
    }
    let state = decode_state(input.u8()?)?;
    let transition = match input.u8()? {
        0 => None,
        1 => Some(MeasurementThresholdTransition::RoseAbove),
        2 => Some(MeasurementThresholdTransition::FellBelow),
        _ => return Err(MeasurementWireRefusal::Malformed),
    };
    if matches!(transition, Some(MeasurementThresholdTransition::RoseAbove))
        && state != MeasurementThresholdState::Above
        || matches!(transition, Some(MeasurementThresholdTransition::FellBelow))
            && state != MeasurementThresholdState::Below
    {
        return Err(MeasurementWireRefusal::Malformed);
    }
    let decision = MeasurementThresholdDecision {
        state,
        transition,
        evaluated_value: input.quantity()?,
        first_observed_at: input.instant()?,
        last_observed_at: input.instant()?,
    };
    if !input.finished() {
        return Err(MeasurementWireRefusal::Malformed);
    }
    encode_measurement_threshold_decision(&decision)?;
    Ok(decision)
}

const fn state_byte(state: MeasurementThresholdState) -> u8 {
    match state {
        MeasurementThresholdState::Below => 0,
        MeasurementThresholdState::Above => 1,
    }
}

fn decode_state(value: u8) -> Result<MeasurementThresholdState, MeasurementWireRefusal> {
    match value {
        0 => Ok(MeasurementThresholdState::Below),
        1 => Ok(MeasurementThresholdState::Above),
        _ => Err(MeasurementWireRefusal::Malformed),
    }
}

fn put_text(output: &mut Vec<u8>, value: &str) -> Result<(), MeasurementWireRefusal> {
    let length =
        u16::try_from(value.len()).map_err(|_| MeasurementWireRefusal::CapacityExceeded)?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn put_instant(
    output: &mut Vec<u8>,
    instant: &TemporalInstant,
) -> Result<(), MeasurementWireRefusal> {
    output.extend_from_slice(&instant.ticks.to_le_bytes());
    output.push(match instant.scale {
        TemporalScale::Seconds => 0,
        TemporalScale::Milliseconds => 1,
        TemporalScale::Microseconds => 2,
        TemporalScale::Nanoseconds => 3,
    });
    put_text(output, &instant.clock_basis)?;
    output.extend_from_slice(&instant.resolution_ticks.to_le_bytes());
    output.extend_from_slice(&instant.uncertainty_ticks.to_le_bytes());
    Ok(())
}

struct Input<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Input<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], MeasurementWireRefusal> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(MeasurementWireRefusal::Malformed)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(MeasurementWireRefusal::Malformed)?;
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, MeasurementWireRefusal> {
        Ok(self.take(1)?[0])
    }
    fn u64(&mut self) -> Result<u64, MeasurementWireRefusal> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| MeasurementWireRefusal::Malformed)?,
        ))
    }
    fn quantity(&mut self) -> Result<Quantity, MeasurementWireRefusal> {
        Quantity::decode(self.take(conduit_core::QUANTITY_ENCODED_LEN)?)
            .map_err(|_| MeasurementWireRefusal::Malformed)
    }
    fn text(&mut self) -> Result<String, MeasurementWireRefusal> {
        let length = usize::from(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| MeasurementWireRefusal::Malformed)?,
        ));
        core::str::from_utf8(self.take(length)?)
            .map(String::from)
            .map_err(|_| MeasurementWireRefusal::Malformed)
    }
    fn instant(&mut self) -> Result<TemporalInstant, MeasurementWireRefusal> {
        let ticks = self.u64()?;
        let scale = match self.u8()? {
            0 => TemporalScale::Seconds,
            1 => TemporalScale::Milliseconds,
            2 => TemporalScale::Microseconds,
            3 => TemporalScale::Nanoseconds,
            _ => return Err(MeasurementWireRefusal::Malformed),
        };
        Ok(TemporalInstant {
            ticks,
            scale,
            clock_basis: self.text()?,
            resolution_ticks: self.u64()?,
            uncertainty_ticks: self.u64()?,
        })
    }
    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
