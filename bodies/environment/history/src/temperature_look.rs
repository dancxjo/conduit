//! Unit presentation is independent of the exact Face and its graph coordinates.
use crate::{GraphRefusal, HistoryGraph};
use alloc::vec::Vec;
use conduit_core::{Quantity, Unit};
use conduit_face::{FormattedTemperature, LookInfo, LookQuantityFacetOption, LookTemperatureUnit};

/// Finite prepared labels over at most 16 retained graph samples. Rounding is
/// once in the selected unit; the original exact quantity remains inspectable.
/// Celsius is this Mask's authored default when a Look has no quantity facet.
pub struct HistoryTemperaturePresentation {
    pub unit: LookTemperatureUnit,
    pub top: FormattedTemperature,
    pub bottom: FormattedTemperature,
    pub samples: Vec<Option<FormattedTemperature>>,
}
impl HistoryTemperaturePresentation {
    pub fn prepare(graph: &HistoryGraph<'_>, look: &LookInfo) -> Result<Self, GraphRefusal> {
        let unit = match look.quantity() {
            LookQuantityFacetOption::Some(facet) => *facet.temperature_unit(),
            LookQuantityFacetOption::None => LookTemperatureUnit::Celsius,
        };
        Self::for_unit(graph, unit)
    }
    pub(crate) fn for_unit(
        graph: &HistoryGraph<'_>,
        unit: LookTemperatureUnit,
    ) -> Result<Self, GraphRefusal> {
        let (low, high) = graph.temperature_range();
        let format = |quantity| {
            FormattedTemperature::prepare(quantity, unit, 3)
                .map_err(|_| GraphRefusal::InvalidProperty)
        };
        let axis = |value| {
            Quantity::from_decimal(i128::from(value), -3, Unit::Celsius)
                .map_err(|_| GraphRefusal::InvalidProperty)
                .and_then(format)
        };
        Ok(Self {
            unit,
            top: axis(high)?,
            bottom: axis(low)?,
            samples: graph
                .samples()
                .iter()
                .map(|sample| sample.temperature_source().map(format).transpose())
                .collect::<Result<_, _>>()?,
        })
    }
    pub fn unit_label(&self) -> &'static str {
        self.top.unit_label()
    }
}
