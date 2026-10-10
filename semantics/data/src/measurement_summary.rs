//! Exact typed summaries derived from a finite measurement window.

use conduit_core::Quantity;

use crate::{BoundedMeasurementWindow, MeasurementSummary, MeasurementSummaryRefusal};

pub const MEASUREMENT_SUMMARY_INFO_ID: &str = "data/measurement-summary@1";

pub fn summarize_measurement_window(
    window: &BoundedMeasurementWindow,
) -> Result<MeasurementSummary, MeasurementSummaryRefusal> {
    let samples = window.samples();
    let first = samples
        .first()
        .ok_or(MeasurementSummaryRefusal::EmptyWindow)?;
    let unit = window.profile().range.minimum.unit();
    if unit.dimension() == conduit_core::QuantityDimension::Temperature {
        return Err(MeasurementSummaryRefusal::TemperatureDifferenceRequired);
    }
    let exponent = samples
        .iter()
        .filter(|sample| sample.value.coefficient() != 0)
        .map(|sample| sample.value.exponent())
        .min()
        .unwrap_or(0);
    let align = |value: Quantity| -> Result<i128, MeasurementSummaryRefusal> {
        if value.coefficient() == 0 {
            return Ok(0);
        }
        let power = u32::try_from(i32::from(value.exponent()) - i32::from(exponent))
            .map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?;
        let scale = 10_i128
            .checked_pow(power)
            .ok_or(MeasurementSummaryRefusal::ArithmeticOverflow)?;
        value
            .coefficient()
            .checked_mul(scale)
            .ok_or(MeasurementSummaryRefusal::ArithmeticOverflow)
    };
    let mut minimum = align(first.value)?;
    let mut maximum = minimum;
    let mut sum = 0_i128;
    for sample in samples {
        if sample.value.unit() != unit {
            return Err(MeasurementSummaryRefusal::UnitMismatch);
        }
        let value = align(sample.value)?;
        minimum = minimum.min(value);
        maximum = maximum.max(value);
        sum = sum
            .checked_add(value)
            .ok_or(MeasurementSummaryRefusal::ArithmeticOverflow)?;
    }
    let sample_count =
        u64::try_from(samples.len()).map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?;
    let divisor = i128::from(sample_count);
    let mut mean_coefficient = sum;
    let mut mean_exponent = exponent;
    // Decimal means are admitted exactly. Nonterminating means refuse.
    let mut remaining = divisor;
    while remaining % 2 == 0 {
        remaining /= 2;
    }
    while remaining % 5 == 0 {
        remaining /= 5;
    }
    if mean_coefficient % remaining != 0 {
        return Err(MeasurementSummaryRefusal::InexactMean);
    }
    while mean_coefficient % divisor != 0 {
        mean_coefficient = mean_coefficient
            .checked_mul(10)
            .ok_or(MeasurementSummaryRefusal::ArithmeticOverflow)?;
        mean_exponent = mean_exponent
            .checked_sub(1)
            .ok_or(MeasurementSummaryRefusal::ArithmeticOverflow)?;
    }
    let mean = Quantity::from_decimal(mean_coefficient / divisor, mean_exponent, unit)
        .map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?;
    let range = maximum
        .checked_sub(minimum)
        .ok_or(MeasurementSummaryRefusal::ArithmeticOverflow)?;
    Ok(MeasurementSummary {
        sample_count,
        first_observed_at: first.observed_at.clone(),
        last_observed_at: samples.last().unwrap().observed_at.clone(),
        minimum: Quantity::from_decimal(minimum, exponent, unit)
            .map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?,
        maximum: Quantity::from_decimal(maximum, exponent, unit)
            .map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?,
        range: Quantity::from_decimal(range, exponent, unit)
            .map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?,
        mean,
    })
}
