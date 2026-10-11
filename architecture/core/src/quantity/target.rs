//! Explicit catalogue-resolved target coordinates. A target coordinate is not
//! a physical quantity tagged with the base unit: its decimal prefix remains
//! part of its meaning and must never be dropped when consumed.

use super::{Quantity, QuantityConversionRefusal, Unit};
use crate::ResolvedQuantitySuffix;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactQuantityTargetCoordinate<'a> {
    target: ResolvedQuantitySuffix<'a>,
    coefficient: i128,
    exponent: i16,
}

impl<'a> ExactQuantityTargetCoordinate<'a> {
    pub(super) fn from_checked_parts(
        target: ResolvedQuantitySuffix<'a>,
        coefficient: i128,
        exponent: i16,
    ) -> Self {
        Self {
            target,
            coefficient,
            exponent,
        }
    }
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

impl Quantity {
    /// Exact coordinate in an owned catalogue-pinned Unit. The returned pair
    /// must remain paired with that Unit; it is not a base-unit quantity.
    pub fn convert_to_unit(
        self,
        target: crate::Unit,
    ) -> Result<(i128, i16), QuantityConversionRefusal> {
        let coordinate = super::wide_conversion::to_target_decimal(self, target, 0)?;
        Ok((coordinate.coefficient(), coordinate.exponent()))
    }

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

pub(super) fn target_parts(target: ResolvedQuantitySuffix<'_>) -> (Unit, i16) {
    (Unit::from_resolved(target), 0)
}
