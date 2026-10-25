//! Admit the shared quantity facet without inventing a physical unit glyph.
use crate::{SevenSegmentFractionMarker, SevenSegmentLook};
use conduit_face::{LookInfo, LookQuantityFacetOption, LookTemperatureUnit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SevenSegmentLookRefusal {
    /// The reviewed seven-segment alphabet has no faithful Kelvin unit glyph.
    UnsupportedTemperatureUnit(LookTemperatureUnit),
}

impl SevenSegmentLook {
    /// Apply only the canonical quantity facet to this authored device policy.
    /// An absent facet preserves its unit; the fraction marker always remains
    /// authored. Other Look facets do not select wiring or change Face truth.
    /// Admission creates no Show or physical output acknowledgement.
    pub fn with_quantity_look(self, look: &LookInfo) -> Result<Self, SevenSegmentLookRefusal> {
        let LookQuantityFacetOption::Some(facet) = look.quantity() else {
            return Ok(self);
        };
        let selected = match facet.temperature_unit() {
            LookTemperatureUnit::Celsius => Self::celsius(),
            LookTemperatureUnit::Fahrenheit => Self::fahrenheit(),
            LookTemperatureUnit::Kelvin => {
                return Err(SevenSegmentLookRefusal::UnsupportedTemperatureUnit(
                    LookTemperatureUnit::Kelvin,
                ));
            }
        };
        Ok(
            if matches!(self.fraction_marker(), SevenSegmentFractionMarker::Colon) {
                selected.with_colon_fraction()
            } else {
                selected
            },
        )
    }
}
