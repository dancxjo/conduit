//! Exact typed summaries derived from a finite measurement window.

use conduit_core::{Quantity, QuantityUnit, TemporalInstant};

use crate::BoundedMeasurementWindow;

pub const MEASUREMENT_SUMMARY_INFO_ID: &str = "data/measurement-summary@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementSummary {
    pub unit: QuantityUnit,
    pub sample_count: u64,
    pub first_observed_at: TemporalInstant,
    pub last_observed_at: TemporalInstant,
    pub minimum: Quantity,
    pub maximum: Quantity,
    pub range: Quantity,
    pub mean: Quantity,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementSummaryRefusal {
    EmptyWindow,
    UnitMismatch,
    ArithmeticOverflow,
    InexactMean,
}

pub fn summarize_measurement_window(
    window: &BoundedMeasurementWindow,
) -> Result<MeasurementSummary, MeasurementSummaryRefusal> {
    let samples = window.samples();
    let first = samples
        .first()
        .ok_or(MeasurementSummaryRefusal::EmptyWindow)?;
    let unit = window.profile().unit;
    let mut minimum = first.value.value();
    let mut maximum = minimum;
    let mut sum = 0_i128;
    for sample in samples {
        if sample.value.unit() != unit {
            return Err(MeasurementSummaryRefusal::UnitMismatch);
        }
        let value = sample.value.value();
        minimum = minimum.min(value);
        maximum = maximum.max(value);
        sum = sum
            .checked_add(i128::from(value))
            .ok_or(MeasurementSummaryRefusal::ArithmeticOverflow)?;
    }
    let sample_count =
        u64::try_from(samples.len()).map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?;
    let divisor = i128::from(sample_count);
    if sum % divisor != 0 {
        return Err(MeasurementSummaryRefusal::InexactMean);
    }
    let mean =
        i64::try_from(sum / divisor).map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?;
    let range = maximum
        .checked_sub(minimum)
        .ok_or(MeasurementSummaryRefusal::ArithmeticOverflow)?;
    Ok(MeasurementSummary {
        unit,
        sample_count,
        first_observed_at: first.observed_at.clone(),
        last_observed_at: samples.last().unwrap().observed_at.clone(),
        minimum: Quantity::new(minimum, unit),
        maximum: Quantity::new(maximum, unit),
        range: Quantity::new(range, unit),
        mean: Quantity::new(mean, unit),
    })
}
