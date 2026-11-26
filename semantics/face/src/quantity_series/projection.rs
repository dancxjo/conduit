//! Finite display projection shared by ordinary Masks. Original facts stay upstream.
use super::PreparedQuantitySeries;
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::Unit;
use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantitySeriesProjectionRefusal {
    Conversion,
    NumericExtent,
    Capacity,
}
impl QuantitySeriesProjectionRefusal {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Conversion => "quantity-series-conversion",
            Self::NumericExtent => "quantity-series-numeric-extent",
            Self::Capacity => "quantity-series-projection-capacity",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QuantitySeriesPoint {
    pub sequence: String,
    pub boot_ms: String,
    pub time_millionths: u32,
    pub value_millionths: Option<u32>,
    pub value: Option<String>,
    pub gap_before: Option<[String; 2]>,
    pub unavailable_reason: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectedQuantitySeries {
    pub source: String,
    pub source_samples: u32,
    pub omitted_points: u32,
    pub unavailable_samples: u32,
    pub gap_runs: u32,
    pub first_boot_ms: Option<String>,
    pub last_boot_ms: Option<String>,
    pub producer: [String; 4],
    pub unit: Option<String>,
    pub minimum: Option<String>,
    pub maximum: Option<String>,
    pub omitted_samples: String,
    pub publication_evidence: Option<String>,
    pub publication_reference: Option<Vec<u8>>,
    pub points: Vec<QuantitySeriesPoint>,
}
impl PreparedQuantitySeries<'_> {
    /// Exact conversion and bounded integer scaling. Unsupported arithmetic
    /// refuses so the Mask can retain the complete original Face reading.
    pub fn project(
        &self,
        selected_unit: Option<Unit>,
        maximum_points: usize,
    ) -> Result<ProjectedQuantitySeries, QuantitySeriesProjectionRefusal> {
        use QuantitySeriesProjectionRefusal as R;
        if maximum_points == 0 || maximum_points > super::MAX_QUANTITY_SERIES_SAMPLES {
            return Err(R::Capacity);
        }
        let unit = selected_unit.or_else(|| {
            self.samples
                .iter()
                .find_map(|sample| sample.value.map(|value| value.unit()))
        });
        let mut values = Vec::with_capacity(self.samples.len());
        for sample in &self.samples {
            values.push(match (sample.value, unit) {
                (Some(value), Some(unit)) => {
                    Some(value.convert_to_unit(unit).map_err(|_| R::Conversion)?)
                }
                _ => None,
            });
        }
        let exponent = values
            .iter()
            .flatten()
            .filter(|(coefficient, _)| *coefficient != 0)
            .map(|(_, exponent)| *exponent)
            .min()
            .unwrap_or(0);
        let align = |coefficient: i128, source_exponent: i16| -> Result<i128, R> {
            if coefficient == 0 {
                return Ok(0);
            }
            let power = u32::try_from(i32::from(source_exponent) - i32::from(exponent))
                .map_err(|_| R::NumericExtent)?;
            coefficient
                .checked_mul(10_i128.checked_pow(power).ok_or(R::NumericExtent)?)
                .ok_or(R::NumericExtent)
        };
        let mut coordinates = Vec::with_capacity(values.len());
        for value in values {
            coordinates.push(
                value
                    .map(|(coefficient, exponent)| align(coefficient, exponent))
                    .transpose()?,
            );
        }
        let range =
            coordinates
                .iter()
                .flatten()
                .fold(None, |range: Option<(i128, i128)>, value| {
                    Some(match range {
                        None => (*value, *value),
                        Some((minimum, maximum)) => (minimum.min(*value), maximum.max(*value)),
                    })
                });
        let mut available = Vec::with_capacity(self.samples.len());
        available.extend(
            self.samples
                .iter()
                .enumerate()
                .filter_map(|(index, sample)| sample.value.map(|_| index)),
        );
        let retained = available.len().min(maximum_points);
        let mut points = Vec::with_capacity(maximum_points);
        for output in 0..retained {
            let selection = if retained == available.len() {
                output
            } else if retained == 1 {
                available.len() - 1
            } else {
                output * (available.len() - 1) / (retained - 1)
            };
            let index = available[selection];
            let sample = &self.samples[index];
            let value_millionths = match (coordinates[index], range) {
                (Some(value), Some((minimum, maximum))) if minimum != maximum => {
                    let span = maximum.checked_sub(minimum).ok_or(R::NumericExtent)?;
                    let offset = value.checked_sub(minimum).ok_or(R::NumericExtent)?;
                    Some(
                        u32::try_from(
                            offset.checked_mul(1_000_000).ok_or(R::NumericExtent)? / span,
                        )
                        .map_err(|_| R::NumericExtent)?,
                    )
                }
                (Some(_), _) => Some(500_000),
                _ => None,
            };
            points.push(QuantitySeriesPoint {
                sequence: sample.sequence.to_string(),
                boot_ms: sample.boot_ms.to_string(),
                time_millionths: self.time_millionths(index).expect("prepared sample index"),
                value_millionths,
                value: coordinates[index]
                    .zip(unit)
                    .map(|(value, unit)| label(value, exponent, unit)),
                gap_before: sample
                    .gap_before
                    .map(|(first, last)| [first.to_string(), last.to_string()]),
                unavailable_reason: sample.unavailable_reason.map(ToString::to_string),
            });
        }
        Ok(ProjectedQuantitySeries {
            source: self.source.into(),
            source_samples: self.samples.len() as u32,
            omitted_points: (available.len() - retained) as u32,
            unavailable_samples: (self.samples.len() - available.len()) as u32,
            gap_runs: self
                .samples
                .iter()
                .filter(|sample| sample.gap_before.is_some())
                .count() as u32,
            first_boot_ms: self
                .samples
                .first()
                .map(|sample| sample.boot_ms.to_string()),
            last_boot_ms: self.samples.last().map(|sample| sample.boot_ms.to_string()),
            producer: self.producer.map(ToString::to_string),
            unit: unit.map(|unit| unit.symbol().into()),
            minimum: range
                .zip(unit)
                .map(|((minimum, _), unit)| label(minimum, exponent, unit)),
            maximum: range
                .zip(unit)
                .map(|((_, maximum), unit)| label(maximum, exponent, unit)),
            omitted_samples: self.omitted_samples.to_string(),
            publication_evidence: self.publication_evidence.map(ToString::to_string),
            publication_reference: self.publication_reference.map(|bytes| bytes.to_vec()),
            points,
        })
    }
}
fn label(coefficient: i128, exponent: i16, unit: Unit) -> String {
    let number = if exponent == 0 || coefficient == 0 {
        coefficient.to_string()
    } else if (1..=6).contains(&exponent) {
        coefficient
            .checked_mul(10_i128.pow(exponent as u32))
            .map(|value| value.to_string())
            .unwrap_or_else(|| format!("{coefficient}e{exponent}"))
    } else if (-6..0).contains(&exponent) {
        let places = (-exponent) as usize;
        let scale = 10_u128.pow(places as u32);
        let magnitude = coefficient.unsigned_abs();
        format!(
            "{}{}.{:0places$}",
            if coefficient < 0 { "-" } else { "" },
            magnitude / scale,
            magnitude % scale
        )
    } else {
        format!("{coefficient}e{exponent}")
    };
    format!("{number} {}", unit.symbol())
}
