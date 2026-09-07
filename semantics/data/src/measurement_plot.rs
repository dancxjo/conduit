//! Finite, source-independent projection of a typed measurement window for plotting.

use alloc::vec::Vec;

use crate::BoundedMeasurementWindow;

pub const MEASUREMENT_PLOT_SERIES_INFO_ID: &str = "data/measurement-plot-series@1";
pub const MAXIMUM_MEASUREMENT_PLOT_POINTS: usize = 32;
pub const PLOT_AXIS_MILLIONTHS: i64 = 1_000_000;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementPlotOverflowPolicy {
    Reject,
    EvenlySpaced,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MeasurementPlotProfile {
    pub point_capacity: usize,
    pub overflow_policy: MeasurementPlotOverflowPolicy,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MeasurementPlotPoint {
    pub source_index: usize,
    pub time_millionths: i64,
    pub value_millionths: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementPlotSeries {
    points: Vec<MeasurementPlotPoint>,
    source_samples: usize,
    omitted_samples: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementPlotRefusal {
    InvalidPointCapacity,
    EmptyWindow,
    Full,
    DegenerateValueRange,
    ArithmeticOverflow,
    InvalidProjection,
}

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
                point.source_index >= source_samples
                    || !(0..=PLOT_AXIS_MILLIONTHS).contains(&point.time_millionths)
                    || !(0..=PLOT_AXIS_MILLIONTHS).contains(&point.value_millionths)
            })
            || points
                .windows(2)
                .any(|pair| pair[0].source_index >= pair[1].source_index)
        {
            return Err(MeasurementPlotRefusal::InvalidProjection);
        }
        Ok(Self {
            points,
            source_samples,
            omitted_samples,
        })
    }

    pub fn project(
        window: &BoundedMeasurementWindow,
        profile: MeasurementPlotProfile,
    ) -> Result<Self, MeasurementPlotRefusal> {
        if profile.point_capacity == 0 || profile.point_capacity > MAXIMUM_MEASUREMENT_PLOT_POINTS {
            return Err(MeasurementPlotRefusal::InvalidPointCapacity);
        }
        let samples = window.samples();
        if samples.is_empty() {
            return Err(MeasurementPlotRefusal::EmptyWindow);
        }
        if samples.len() > profile.point_capacity
            && profile.overflow_policy == MeasurementPlotOverflowPolicy::Reject
        {
            return Err(MeasurementPlotRefusal::Full);
        }
        let minimum = window.profile().range.minimum.value();
        let maximum = window.profile().range.maximum.value();
        let span = maximum
            .checked_sub(minimum)
            .ok_or(MeasurementPlotRefusal::ArithmeticOverflow)?;
        if span == 0 {
            return Err(MeasurementPlotRefusal::DegenerateValueRange);
        }

        let retained = samples.len().min(profile.point_capacity);
        let mut points = Vec::with_capacity(profile.point_capacity);
        for output_index in 0..retained {
            let source_index = selected_index(output_index, retained, samples.len());
            let value_offset = samples[source_index]
                .value
                .value()
                .checked_sub(minimum)
                .ok_or(MeasurementPlotRefusal::ArithmeticOverflow)?;
            let value_millionths = scaled(value_offset, span)?;
            let time_millionths = if samples.len() == 1 {
                0
            } else {
                scaled(source_index as i64, (samples.len() - 1) as i64)?
            };
            points.push(MeasurementPlotPoint {
                source_index,
                time_millionths,
                value_millionths,
            });
        }
        Ok(Self {
            points,
            source_samples: samples.len(),
            omitted_samples: samples.len() - retained,
        })
    }

    pub fn points(&self) -> &[MeasurementPlotPoint] {
        &self.points
    }

    pub const fn source_samples(&self) -> usize {
        self.source_samples
    }

    pub const fn omitted_samples(&self) -> usize {
        self.omitted_samples
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

fn scaled(numerator: i64, denominator: i64) -> Result<i64, MeasurementPlotRefusal> {
    let value = i128::from(numerator)
        .checked_mul(i128::from(PLOT_AXIS_MILLIONTHS))
        .ok_or(MeasurementPlotRefusal::ArithmeticOverflow)?
        / i128::from(denominator);
    i64::try_from(value).map_err(|_| MeasurementPlotRefusal::ArithmeticOverflow)
}
