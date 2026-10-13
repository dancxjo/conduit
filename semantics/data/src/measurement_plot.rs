//! Finite, source-independent projection of a typed measurement window for plotting.

use alloc::vec::Vec;
use conduit_plot::rust_binding::BoundedSequence;

use crate::{
    BoundedMeasurementWindow, MeasurementPlotOverflowPolicy, MeasurementPlotPoint,
    MeasurementPlotProfile, MeasurementPlotRefusal, MeasurementPlotSeries,
};

pub const MEASUREMENT_PLOT_SERIES_INFO_ID: &str = "data/measurement-plot-series@1";
pub const MAXIMUM_MEASUREMENT_PLOT_POINTS: usize = 32;
pub const PLOT_AXIS_MILLIONTHS: i64 = 1_000_000;

impl MeasurementPlotSeries {
    pub fn from_projected(
        points: Vec<MeasurementPlotPoint>,
        source_samples: usize,
        omitted_samples: usize,
    ) -> Result<Self, MeasurementPlotRefusal> {
        if points.is_empty()
            || points.len() > MAXIMUM_MEASUREMENT_PLOT_POINTS
            || source_samples.checked_sub(points.len()) != Some(omitted_samples)
            || points.iter().any(|point| {
                usize::try_from(*point.source_index()).map_or(true, |index| index >= source_samples)
                    || !(0..=PLOT_AXIS_MILLIONTHS).contains(point.time_millionths())
                    || !(0..=PLOT_AXIS_MILLIONTHS).contains(point.value_millionths())
            })
            || points
                .windows(2)
                .any(|pair| pair[0].source_index() >= pair[1].source_index())
        {
            return Err(MeasurementPlotRefusal::InvalidProjection);
        }
        let points = BoundedSequence::try_from_iter(points)
            .map_err(|_| MeasurementPlotRefusal::InvalidProjection)?;
        Self::new(points, source_samples as u64, omitted_samples as u64)
            .map_err(|_| MeasurementPlotRefusal::InvalidProjection)
    }

    pub fn project(
        window: &BoundedMeasurementWindow,
        profile: MeasurementPlotProfile,
    ) -> Result<Self, MeasurementPlotRefusal> {
        let point_capacity = usize::from(*profile.point_capacity());
        if point_capacity == 0 || point_capacity > MAXIMUM_MEASUREMENT_PLOT_POINTS {
            return Err(MeasurementPlotRefusal::InvalidPointCapacity);
        }
        let samples = window.samples();
        if samples.is_empty() {
            return Err(MeasurementPlotRefusal::EmptyWindow);
        }
        if samples.len() > point_capacity
            && *profile.overflow_policy() == MeasurementPlotOverflowPolicy::Reject
        {
            return Err(MeasurementPlotRefusal::Full);
        }
        let unit = window.profile().range.minimum.unit().to_owned();
        let minimum = window.profile().range.minimum.coordinate().to_owned()
            .map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)?;
        let maximum = window.profile().range.maximum.convert_to_unit(&unit)
            .map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)?;
        let span = maximum.checked_sub(&minimum)
            .map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)?;
        if span.mathematical_equality(&conduit_core::MathValue::ZERO)
            .map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)? == conduit_core::MathematicalEquality::Equal {
            return Err(MeasurementPlotRefusal::DegenerateValueRange);
        }

        let retained = samples.len().min(point_capacity);
        let mut points = Vec::with_capacity(point_capacity);
        for output_index in 0..retained {
            let source_index = selected_index(output_index, retained, samples.len());
            let coordinate = samples[source_index].value.convert_to_unit(&unit)
                .map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)?;
            let projected = coordinate.checked_sub(&minimum)
                .and_then(|offset| offset.checked_div(&span))
                .and_then(|ratio| ratio.checked_mul(&conduit_core::MathValue::integer(PLOT_AXIS_MILLIONTHS)))
                .map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)?;
            // This plot profile explicitly projects to a finite millionth grid.
            let value_millionths = projected.checked_floor_i64()
                .map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)?;
            let time_millionths = if samples.len() == 1 {
                0
            } else {
                scaled(source_index as i128, (samples.len() - 1) as i128)?
            };
            points.push(
                MeasurementPlotPoint::new(source_index as u64, time_millionths, value_millionths)
                    .expect("projection establishes native point bounds"),
            );
        }
        Self::from_projected(points, samples.len(), samples.len() - retained)
    }

    pub fn points(&self) -> &BoundedSequence<MeasurementPlotPoint, 32> {
        self.projected_points()
    }

    pub fn point(&self, index: usize) -> Option<&MeasurementPlotPoint> {
        self.projected_points().iter().nth(index)
    }

    pub fn source_samples(&self) -> usize {
        *self.source_sample_count() as usize
    }

    pub fn omitted_samples(&self) -> usize {
        *self.omitted_sample_count() as usize
    }
}

fn selected_index(output_index: usize, retained: usize, source: usize) -> usize {
    if retained == source {
        output_index
    } else if retained == 1 {
        source - 1
    } else {
        output_index * (source - 1) / (retained - 1)
    }
}

fn scaled(numerator: i128, denominator: i128) -> Result<i64, MeasurementPlotRefusal> {
    let value = numerator
        .checked_mul(i128::from(PLOT_AXIS_MILLIONTHS))
        .ok_or(MeasurementPlotRefusal::ArithmeticOverflow)?
        / denominator;
    i64::try_from(value).map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)
}
