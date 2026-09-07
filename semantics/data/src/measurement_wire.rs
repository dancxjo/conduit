//! Finite canonical payloads for the measurement leaf types.

use alloc::{string::String, vec::Vec};
use conduit_core::{Quantity, TemporalInstant, TemporalScale};

use crate::{
    BoundedMeasurementWindow, FullWindowPolicy, MeasurementPlotPoint, MeasurementPlotSeries,
    MeasurementRange, MeasurementSample, MeasurementWindowProfile, MAXIMUM_MEASUREMENT_PLOT_POINTS,
    MAXIMUM_MEASUREMENT_WINDOW_SAMPLES,
};

pub const MAXIMUM_MEASUREMENT_WINDOW_BYTES: usize = 32_768;
pub const MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES: usize = 1_024;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementWireRefusal {
    Malformed,
    UnsupportedVersion,
    CapacityExceeded,
    InvalidWindow,
}

pub fn encode_measurement_window(
    window: &BoundedMeasurementWindow,
) -> Result<Vec<u8>, MeasurementWireRefusal> {
    let profile = window.profile();
    let mut bytes = Vec::with_capacity(MAXIMUM_MEASUREMENT_WINDOW_BYTES.min(256));
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
    bytes.extend_from_slice(&window.discarded_samples().to_le_bytes());
    bytes.push(
        u8::try_from(window.samples().len())
            .map_err(|_| MeasurementWireRefusal::CapacityExceeded)?,
    );
    for sample in window.samples() {
        bytes.extend_from_slice(&sample.value.encode());
        put_instant(&mut bytes, &sample.observed_at)?;
        match sample.uncertainty {
            Some(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.encode());
            }
            None => bytes.push(0),
        }
    }
    if bytes.len() > MAXIMUM_MEASUREMENT_WINDOW_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    Ok(bytes)
}

pub fn decode_measurement_window(
    bytes: &[u8],
) -> Result<BoundedMeasurementWindow, MeasurementWireRefusal> {
    if bytes.len() > MAXIMUM_MEASUREMENT_WINDOW_BYTES {
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
    let clock_basis = input.text()?;
    let discarded_samples = input.u64()?;
    let count = usize::from(input.u8()?);
    if count > capacity {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    let unit = minimum.unit();
    let profile = MeasurementWindowProfile {
        capacity,
        unit,
        range: MeasurementRange { minimum, maximum },
        clock_basis,
        full_policy,
    };
    let mut samples = Vec::with_capacity(capacity);
    for _ in 0..count {
        let value = input.quantity()?;
        let observed_at = input.instant()?;
        let uncertainty = match input.u8()? {
            0 => None,
            1 => Some(input.quantity()?),
            _ => return Err(MeasurementWireRefusal::Malformed),
        };
        samples.push(MeasurementSample {
            value,
            observed_at,
            uncertainty,
        });
    }
    if !input.finished() {
        return Err(MeasurementWireRefusal::Malformed);
    }
    BoundedMeasurementWindow::from_retained(profile, samples, discarded_samples)
        .map_err(|_| MeasurementWireRefusal::InvalidWindow)
}

pub fn encode_measurement_plot_series(
    series: &MeasurementPlotSeries,
) -> Result<Vec<u8>, MeasurementWireRefusal> {
    let mut bytes = Vec::with_capacity(MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES.min(256));
    bytes.push(1);
    bytes.push(
        u8::try_from(series.points().len())
            .map_err(|_| MeasurementWireRefusal::CapacityExceeded)?,
    );
    bytes.extend_from_slice(&(series.source_samples() as u64).to_le_bytes());
    bytes.extend_from_slice(&(series.omitted_samples() as u64).to_le_bytes());
    for point in series.points() {
        bytes.extend_from_slice(&(point.source_index as u64).to_le_bytes());
        bytes.extend_from_slice(&point.time_millionths.to_le_bytes());
        bytes.extend_from_slice(&point.value_millionths.to_le_bytes());
    }
    if bytes.len() > MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    Ok(bytes)
}

pub fn decode_measurement_plot_series(
    bytes: &[u8],
) -> Result<MeasurementPlotSeries, MeasurementWireRefusal> {
    let mut input = Input::new(bytes);
    if input.u8()? != 1 {
        return Err(MeasurementWireRefusal::UnsupportedVersion);
    }
    let count = usize::from(input.u8()?);
    if count > MAXIMUM_MEASUREMENT_PLOT_POINTS {
        return Err(MeasurementWireRefusal::CapacityExceeded);
    }
    let source_samples =
        usize::try_from(input.u64()?).map_err(|_| MeasurementWireRefusal::CapacityExceeded)?;
    let omitted_samples =
        usize::try_from(input.u64()?).map_err(|_| MeasurementWireRefusal::CapacityExceeded)?;
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        points.push(MeasurementPlotPoint {
            source_index: usize::try_from(input.u64()?)
                .map_err(|_| MeasurementWireRefusal::CapacityExceeded)?,
            time_millionths: input.i64()?,
            value_millionths: input.i64()?,
        });
    }
    if !input.finished() {
        return Err(MeasurementWireRefusal::Malformed);
    }
    MeasurementPlotSeries::from_projected(points, source_samples, omitted_samples)
        .map_err(|_| MeasurementWireRefusal::Malformed)
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
    value: &TemporalInstant,
) -> Result<(), MeasurementWireRefusal> {
    output.extend_from_slice(&value.ticks.to_le_bytes());
    output.push(match value.scale {
        TemporalScale::Seconds => 0,
        TemporalScale::Milliseconds => 1,
        TemporalScale::Microseconds => 2,
        TemporalScale::Nanoseconds => 3,
    });
    put_text(output, &value.clock_basis)?;
    output.extend_from_slice(&value.resolution_ticks.to_le_bytes());
    output.extend_from_slice(&value.uncertainty_ticks.to_le_bytes());
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
    fn i64(&mut self) -> Result<i64, MeasurementWireRefusal> {
        Ok(i64::from_le_bytes(
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

    fn sample(value: i64, ticks: u64) -> MeasurementSample {
        MeasurementSample {
            value: Quantity::new(value, conduit_core::QuantityUnit::Millivolt),
            observed_at: TemporalInstant {
                ticks,
                scale: TemporalScale::Milliseconds,
                clock_basis: "wire-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            uncertainty: Some(Quantity::new(1, conduit_core::QuantityUnit::Millivolt)),
        }
    }

    #[test]
    fn bounded_window_and_plot_payloads_round_trip_exactly() {
        let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
            capacity: 2,
            unit: conduit_core::QuantityUnit::Millivolt,
            range: MeasurementRange {
                minimum: Quantity::new(-100, conduit_core::QuantityUnit::Millivolt),
                maximum: Quantity::new(100, conduit_core::QuantityUnit::Millivolt),
            },
            clock_basis: "wire-clock".into(),
            full_policy: FullWindowPolicy::DropOldest,
        })
        .unwrap();
        for (value, ticks) in [(-100, 1), (0, 2), (100, 3)] {
            window.push(sample(value, ticks)).unwrap();
        }
        let restored =
            decode_measurement_window(&encode_measurement_window(&window).unwrap()).unwrap();
        assert_eq!(restored, window);

        let series = MeasurementPlotSeries::project(
            &window,
            crate::MeasurementPlotProfile {
                point_capacity: 2,
                overflow_policy: crate::MeasurementPlotOverflowPolicy::EvenlySpaced,
            },
        )
        .unwrap();
        let decoded =
            decode_measurement_plot_series(&encode_measurement_plot_series(&series).unwrap())
                .unwrap();
        assert_eq!(decoded, series);
    }

    #[test]
    fn payload_decoder_rejects_truncation_and_trailing_bytes() {
        let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
            capacity: 1,
            unit: conduit_core::QuantityUnit::Millivolt,
            range: MeasurementRange {
                minimum: Quantity::new(0, conduit_core::QuantityUnit::Millivolt),
                maximum: Quantity::new(100, conduit_core::QuantityUnit::Millivolt),
            },
            clock_basis: "wire-clock".into(),
            full_policy: FullWindowPolicy::Reject,
        })
        .unwrap();
        window.push(sample(50, 1)).unwrap();
        let mut bytes = encode_measurement_window(&window).unwrap();
        assert_eq!(
            decode_measurement_window(&bytes[..bytes.len() - 1]),
            Err(MeasurementWireRefusal::Malformed)
        );
        bytes.push(0);
        assert_eq!(
            decode_measurement_window(&bytes),
            Err(MeasurementWireRefusal::Malformed)
        );
    }
}
