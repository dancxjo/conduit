//! Bounded labelled normalized series, not producer identity or clock alignment.
//! These preparation codecs allocate bounded intermediates outside kernel Step.
use crate::MeasurementPlotSeries;
use alloc::{string::String, vec::Vec};
use conduit_plot::rust_binding::BoundedSequence;
mod wire;
pub use wire::{decode_measurement_chart, encode_measurement_chart};
pub const MEASUREMENT_CHART_INFO_ID: &str = "data/measurement-chart@1";
pub const MAXIMUM_MEASUREMENT_CHART_SERIES: usize = 4;
pub const MAXIMUM_MEASUREMENT_CHART_LABEL_BYTES: usize = 64;
pub const MAXIMUM_MEASUREMENT_CHART_BYTES: usize = 2 + MAXIMUM_MEASUREMENT_CHART_SERIES
    * (1 + MAXIMUM_MEASUREMENT_CHART_LABEL_BYTES
        + 2
        + crate::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasurementChartRefusal {
    Count,
    InvalidLabel,
    DuplicateLabel,
    InvalidSeries,
    Encoding,
    UnsupportedVersion,
    Capacity,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelledMeasurementSeries {
    label: String,
    series: MeasurementPlotSeries,
}
impl LabelledMeasurementSeries {
    pub fn new(
        label: String,
        series: MeasurementPlotSeries,
    ) -> Result<Self, MeasurementChartRefusal> {
        if !valid_label(&label) {
            return Err(MeasurementChartRefusal::InvalidLabel);
        }
        // Generated leaf constructors alone do not prove projection invariants.
        let bytes = crate::encode_measurement_plot_series(&series)
            .map_err(|_| MeasurementChartRefusal::InvalidSeries)?;
        crate::decode_measurement_plot_series(&bytes)
            .map_err(|_| MeasurementChartRefusal::InvalidSeries)?;
        let label = if label.capacity() > MAXIMUM_MEASUREMENT_CHART_LABEL_BYTES {
            String::from(label.as_str())
        } else {
            label
        };
        Ok(Self { label, series })
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn series(&self) -> &MeasurementPlotSeries {
        &self.series
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasurementChart {
    series: BoundedSequence<LabelledMeasurementSeries, MAXIMUM_MEASUREMENT_CHART_SERIES>,
}
impl MeasurementChart {
    /// Backing capacity must also be <=4; no hidden oversized allocation is retained.
    /// Author order is retained. Human names convey no authenticated Host/Boot,
    /// source unit/range, wall time or cross-window alignment.
    pub fn new(series: Vec<LabelledMeasurementSeries>) -> Result<Self, MeasurementChartRefusal> {
        if series.is_empty() || series.len() > MAXIMUM_MEASUREMENT_CHART_SERIES {
            return Err(MeasurementChartRefusal::Count);
        }
        for (index, item) in series.iter().enumerate() {
            if series[..index]
                .iter()
                .any(|prior| prior.label == item.label)
            {
                return Err(MeasurementChartRefusal::DuplicateLabel);
            }
        }
        Ok(Self {
            series: BoundedSequence::try_from_preallocated_vec(series)
                .map_err(|_| MeasurementChartRefusal::Capacity)?,
        })
    }
    pub fn series(&self) -> &[LabelledMeasurementSeries] {
        self.series.as_slice()
    }
}
fn valid_label(label: &str) -> bool {
    !label.trim().is_empty()
        && label.len() <= MAXIMUM_MEASUREMENT_CHART_LABEL_BYTES
        && !label.chars().any(char::is_control)
}
pub fn measurement_chart_type() -> conduit_core::StructuredInfoType {
    conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(MEASUREMENT_CHART_INFO_ID))
        .expect("bounded chart leaf")
}
pub fn measurement_chart_canonical_bytes() -> usize {
    measurement_chart_type()
        .canonical_bytes()
        .expect("chart type")
        .len()
        + 5
        + MAXIMUM_MEASUREMENT_CHART_BYTES
}
/// Conservative requested backing for decode plus all retained labels/points
/// and one full canonical chart payload. Allocator overhead, caller input,
/// stacks and Face construction remain separately charged. This is no lease.
pub fn measurement_chart_decode_workspace_bytes() -> usize {
    core::mem::size_of::<MeasurementChart>()
        + MAXIMUM_MEASUREMENT_CHART_SERIES
            * (core::mem::size_of::<LabelledMeasurementSeries>()
                + MAXIMUM_MEASUREMENT_CHART_LABEL_BYTES
                + 2 * crate::MAXIMUM_MEASUREMENT_PLOT_POINTS
                    * core::mem::size_of::<crate::MeasurementPlotPoint>())
        + crate::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES
        + measurement_chart_canonical_bytes()
}
#[cfg(test)]
mod tests;
