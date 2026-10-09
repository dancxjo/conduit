//! Explicit catalogue-resolved target coordinates. A target coordinate is not
//! a physical quantity tagged with the base unit: its decimal prefix remains
//! part of its meaning and must never be dropped when consumed.

use super::{ExactDecimalQuantity, QuantityConversionRefusal, QuantityUnit};
use crate::ResolvedQuantitySuffix;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactQuantityTargetCoordinate<'a> {
    target: ResolvedQuantitySuffix<'a>,
    coefficient: i128,
    exponent: i16,
}

impl<'a> ExactQuantityTargetCoordinate<'a> {
    pub const fn target(self) -> ResolvedQuantitySuffix<'a> {
        self.target
    }
    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }
    pub const fn exponent(self) -> i16 {
        self.exponent
    }
}

impl ExactDecimalQuantity {
    /// Resolve the target separately using the pinned whole-suffix catalogue.
    /// Prefix scaling applies to its coordinate, never to an affine offset.
    /// The returned coordinate retains the complete target descriptor rather
    /// than masquerading as a quantity in the unprefixed base unit.
    pub fn convert_to_target<'a>(
        self,
        target: ResolvedQuantitySuffix<'a>,
    ) -> Result<ExactQuantityTargetCoordinate<'a>, QuantityConversionRefusal> {
        let (unit, exponent) = target_parts(target);
        let coordinate = super::wide_conversion::to_target_decimal(self, unit, exponent)?;
        Ok(ExactQuantityTargetCoordinate {
            target,
            coefficient: coordinate.coefficient(),
            exponent: coordinate.exponent(),
        })
    }
}

pub(super) fn target_parts(target: ResolvedQuantitySuffix<'_>) -> (QuantityUnit, i16) {
    match target.base() {
        Some(base) => (base.unit(), target.decimal_exponent().unwrap()),
        None => (target.legacy_unit().unwrap(), 0),
    }
}
