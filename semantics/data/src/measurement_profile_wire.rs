//! Finite canonical payloads that configure and feed a measurement window.

use alloc::{string::String, vec::Vec};
use conduit_core::{Quantity, TemporalInstant, TemporalScale};

use crate::{
    FullWindowPolicy, MeasurementRange, MeasurementSample, MeasurementWindowProfile,
    MeasurementWireRefusal, MAXIMUM_MEASUREMENT_WINDOW_SAMPLES,
};

pub const MAXIMUM_MEASUREMENT_WINDOW_PROFILE_BYTES: usize = 1_024;
pub const MAXIMUM_MEASUREMENT_SAMPLE_BYTES: usize = 1_024;

pub fn encode_measurement_window_profile(
    profile: &MeasurementWindowProfile,
) -> Result<Vec<u8>, MeasurementWireRefusal> {
    profile
        .validate()
        .map_err(|_| MeasurementWireRefusal::InvalidWindow)?;
    let mut bytes = Vec::with_capacity(64 + profile.clock_basis.len());
    bytes.push(1);
    bytes.push(
        u8::try_from(profile.capacity).map_err(|_| MeasurementWireRefusal::CapacityExceeded)?,
    );
    bytes.push(match profile.full_policy {
        FullWindowPolicy::Reject => 0,
        FullWindowPolicy::DropOldest => 1,
    });
    bytes.extend_from_slice(&profile.range.minimum.encode());
    bytes.extend_from_slice(&profile.range.maximum.encode());
    put_text(&mut bytes, &profile.clock_basis)?;
    if bytes.len() > MAXIMUM_MEASUREMENT_WINDOW_PROFILE_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    Ok(bytes)
}

pub fn decode_measurement_window_profile(
    bytes: &[u8],
) -> Result<MeasurementWindowProfile, MeasurementWireRefusal> {
    if bytes.len() > MAXIMUM_MEASUREMENT_WINDOW_PROFILE_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    let mut input = Input::new(bytes);
    if input.u8()? != 1 {
        return Err(MeasurementWireRefusal::UnsupportedVersion);
    }
    let capacity = usize::from(input.u8()?);
    if capacity == 0 || capacity > MAXIMUM_MEASUREMENT_WINDOW_SAMPLES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    let full_policy = match input.u8()? {
        0 => FullWindowPolicy::Reject,
        1 => FullWindowPolicy::DropOldest,
        _ => return Err(MeasurementWireRefusal::Malformed),
    };
    let minimum = input.quantity()?;
    let maximum = input.quantity()?;
    let profile = MeasurementWindowProfile {
        capacity,
        unit: minimum.unit(),
        range: MeasurementRange { minimum, maximum },
        clock_basis: input.text()?,
        full_policy,
    };
    if !input.finished() {
        return Err(MeasurementWireRefusal::Malformed);
    }
    profile
        .validate()
        .map_err(|_| MeasurementWireRefusal::InvalidWindow)?;
    Ok(profile)
}

pub fn encode_measurement_sample(
    sample: &MeasurementSample,
) -> Result<Vec<u8>, MeasurementWireRefusal> {
    sample
        .observed_at
        .validate()
        .map_err(|_| MeasurementWireRefusal::Malformed)?;
    if sample.uncertainty.is_some_and(|uncertainty| {
        uncertainty.unit() != sample.value.unit() || uncertainty.value() < 0
    }) {
        return Err(MeasurementWireRefusal::Malformed);
    }
    let mut bytes = Vec::with_capacity(64 + sample.observed_at.clock_basis.len());
    bytes.push(1);
    bytes.extend_from_slice(&sample.value.encode());
    put_instant(&mut bytes, &sample.observed_at)?;
    match sample.uncertainty {
        Some(uncertainty) => {
            bytes.push(1);
            bytes.extend_from_slice(&uncertainty.encode());
        }
        None => bytes.push(0),
    }
    if bytes.len() > MAXIMUM_MEASUREMENT_SAMPLE_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    Ok(bytes)
}

pub fn decode_measurement_sample(
    bytes: &[u8],
) -> Result<MeasurementSample, MeasurementWireRefusal> {
    if bytes.len() > MAXIMUM_MEASUREMENT_SAMPLE_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    let mut input = Input::new(bytes);
    if input.u8()? != 1 {
        return Err(MeasurementWireRefusal::UnsupportedVersion);
    }
    let value = input.quantity()?;
    let observed_at = input.instant()?;
    let uncertainty = match input.u8()? {
        0 => None,
        1 => Some(input.quantity()?),
        _ => return Err(MeasurementWireRefusal::Malformed),
    };
    if !input.finished() {
        return Err(MeasurementWireRefusal::Malformed);
    }
    let sample = MeasurementSample {
        value,
        observed_at,
        uncertainty,
    };
    encode_measurement_sample(&sample)?;
    Ok(sample)
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::QuantityUnit;

    #[test]
    fn profile_and_sample_payloads_round_trip_without_host_defaults() {
        let profile = MeasurementWindowProfile {
            capacity: 8,
            unit: QuantityUnit::Millivolt,
            range: MeasurementRange {
                minimum: Quantity::new(-100, QuantityUnit::Millivolt),
                maximum: Quantity::new(100, QuantityUnit::Millivolt),
            },
            clock_basis: "source-clock".into(),
            full_policy: FullWindowPolicy::DropOldest,
        };
        assert_eq!(
            decode_measurement_window_profile(
                &encode_measurement_window_profile(&profile).unwrap()
            ),
            Ok(profile)
        );
        let sample = MeasurementSample {
            value: Quantity::new(25, QuantityUnit::Millivolt),
            observed_at: TemporalInstant {
                ticks: 10,
                scale: TemporalScale::Milliseconds,
                clock_basis: "source-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            uncertainty: Some(Quantity::new(1, QuantityUnit::Millivolt)),
        };
        assert_eq!(
            decode_measurement_sample(&encode_measurement_sample(&sample).unwrap()),
            Ok(sample)
        );
    }
}
