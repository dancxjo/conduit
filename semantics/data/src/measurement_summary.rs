//! Exact typed summaries derived from a finite measurement window.
use conduit_core::{MathValue, Quantity};
use core::cmp::Ordering;
use crate::{BoundedMeasurementWindow, MeasurementSummary, MeasurementSummaryRefusal};
pub const MEASUREMENT_SUMMARY_INFO_ID: &str = "data/measurement-summary@1";
pub fn summarize_measurement_window(window: &BoundedMeasurementWindow) -> Result<MeasurementSummary, MeasurementSummaryRefusal> {
    let samples = window.samples();
    let first = samples.first().ok_or(MeasurementSummaryRefusal::EmptyWindow)?;
    if first.value.role() == conduit_core::QuantityRole::Point { return Err(MeasurementSummaryRefusal::PointDifferenceRequired); }
    let unit = first.value.unit().to_owned();
    let numeric = |_| MeasurementSummaryRefusal::ArithmeticOverflow;
    let mut minimum = first.value.coordinate().to_owned().map_err(numeric)?;
    let mut maximum = minimum.clone();
    let mut sum = MathValue::ZERO;
    for sample in samples {
        if !sample.value.unit().same_physical_definition(unit.as_ref()) { return Err(MeasurementSummaryRefusal::UnitMismatch); }
        let coordinate = sample.value.convert_to_unit(&unit).map_err(|_| MeasurementSummaryRefusal::UnitMismatch)?;
        match coordinate.compare(&minimum).map_err(numeric)? {
            Some(Ordering::Less) => minimum = coordinate.clone(),
            None => return Err(MeasurementSummaryRefusal::ArithmeticOverflow),
            _ => {}
        }
        match coordinate.compare(&maximum).map_err(numeric)? {
            Some(Ordering::Greater) => maximum = coordinate.clone(),
            None => return Err(MeasurementSummaryRefusal::ArithmeticOverflow),
            _ => {}
        }
        sum = sum.checked_add(&coordinate).map_err(numeric)?;
    }
    let sample_count = u64::try_from(samples.len()).map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?;
    let mean = sum.checked_div(&MathValue::integer(sample_count as i64)).map_err(numeric)?;
    let range = maximum.checked_sub(&minimum).map_err(numeric)?;
    let quantity = |value| {
        let quantity = Quantity::from_math_role(value, &unit, first.value.role()).map_err(|_| MeasurementSummaryRefusal::ArithmeticOverflow)?;
        if quantity.encoded().len() > 2048 { return Err(MeasurementSummaryRefusal::ArithmeticOverflow); }
        Ok(quantity)
    };
    Ok(MeasurementSummary {
        sample_count,
        first_observed_at: first.observed_at.clone(),
        last_observed_at: samples.last().unwrap().observed_at.clone(),
        minimum: quantity(minimum)?, maximum: quantity(maximum)?, range: quantity(range)?, mean: quantity(mean)?,
    })
}
