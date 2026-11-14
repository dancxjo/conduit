//! Versioned intrinsic payload; each series uses the existing exact codec.
use super::*;
use alloc::string::ToString;
pub fn encode_measurement_chart(
    chart: &MeasurementChart,
) -> Result<Vec<u8>, MeasurementChartRefusal> {
    let mut output = Vec::with_capacity(MAXIMUM_MEASUREMENT_CHART_BYTES);
    output.extend_from_slice(&[1, chart.series().len() as u8]);
    for item in chart.series() {
        let bytes = crate::encode_measurement_plot_series(item.series())
            .map_err(|_| MeasurementChartRefusal::InvalidSeries)?;
        if bytes.len() > crate::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES {
            return Err(MeasurementChartRefusal::Capacity);
        }
        output.push(item.label().len() as u8);
        output.extend_from_slice(item.label().as_bytes());
        output.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        output.extend_from_slice(&bytes);
    }
    Ok(output)
}
pub fn decode_measurement_chart(bytes: &[u8]) -> Result<MeasurementChart, MeasurementChartRefusal> {
    use MeasurementChartRefusal as R;
    if bytes.len() > MAXIMUM_MEASUREMENT_CHART_BYTES {
        return Err(R::Capacity);
    }
    let mut input = Input(bytes);
    if input.byte()? != 1 {
        return Err(R::UnsupportedVersion);
    }
    let count = usize::from(input.byte()?);
    if count == 0 || count > MAXIMUM_MEASUREMENT_CHART_SERIES {
        return Err(R::Count);
    }
    let mut values: Vec<LabelledMeasurementSeries> = Vec::with_capacity(count);
    for _ in 0..count {
        let label_length = usize::from(input.byte()?);
        if label_length == 0 || label_length > MAXIMUM_MEASUREMENT_CHART_LABEL_BYTES {
            return Err(R::InvalidLabel);
        }
        let label = core::str::from_utf8(input.take(label_length)?).map_err(|_| R::InvalidLabel)?;
        if !valid_label(label) {
            return Err(R::InvalidLabel);
        }
        if values.iter().any(|prior| prior.label() == label) {
            return Err(R::DuplicateLabel);
        }
        let length =
            u16::from_le_bytes(input.take(2)?.try_into().map_err(|_| R::Encoding)?) as usize;
        if length > crate::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES {
            return Err(R::Capacity);
        }
        let series = crate::decode_measurement_plot_series(input.take(length)?)
            .map_err(|_| R::InvalidSeries)?;
        // Already validated by the shared decoder, without a second allocation.
        values.push(LabelledMeasurementSeries {
            label: label.to_string(),
            series,
        });
    }
    if !input.0.is_empty() {
        return Err(R::Encoding);
    }
    MeasurementChart::new(values)
}
struct Input<'a>(&'a [u8]);
impl<'a> Input<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], MeasurementChartRefusal> {
        let (value, remaining) = self
            .0
            .split_at_checked(count)
            .ok_or(MeasurementChartRefusal::Encoding)?;
        self.0 = remaining;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, MeasurementChartRefusal> {
        Ok(self.take(1)?[0])
    }
}
