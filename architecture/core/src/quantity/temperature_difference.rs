//! Explicit temperature differences share the bounded decimal coordinate law,
//! but never acquire an absolute point's offset or its semantic identity.
//! The Plot representation wraps the coordinate in a distinct checked record;
//! the point codec is not a standalone difference encoding.

use super::{
    ExactDecimalQuantity, ExactDecimalQuantityRefusal, ExactQuantityConversionRequestRefusal,
    QuantityConversionRefusal, QuantityDimension, QuantityUnit,
};
use crate::ResolvedQuantitySuffix;
use core::cmp::Ordering;

pub const EXACT_TEMPERATURE_DIFFERENCE_INFO_ID: &str = "quantity/exact-temperature-difference@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExactTemperatureDifferenceRefusal {
    Coordinate(ExactDecimalQuantityRefusal),
    NotTemperature,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactTemperatureDifference {
    coordinate: ExactDecimalQuantity,
}

impl ExactTemperatureDifference {
    pub fn new(
        coefficient: i128,
        exponent: i16,
        unit: QuantityUnit,
    ) -> Result<Self, ExactTemperatureDifferenceRefusal> {
        Self::admit(
            ExactDecimalQuantity::new(coefficient, exponent, unit)
                .map_err(ExactTemperatureDifferenceRefusal::Coordinate)?,
        )
    }

    pub fn parse_plot_literal(literal: &str) -> Result<Self, ExactTemperatureDifferenceRefusal> {
        Self::admit(
            ExactDecimalQuantity::parse_plot_literal(literal)
                .map_err(ExactTemperatureDifferenceRefusal::Coordinate)?,
        )
    }

    fn admit(coordinate: ExactDecimalQuantity) -> Result<Self, ExactTemperatureDifferenceRefusal> {
        if coordinate.dimension() != QuantityDimension::Temperature {
            return Err(ExactTemperatureDifferenceRefusal::NotTemperature);
        }
        Ok(Self { coordinate })
    }

    /// Raw storage coordinates require the owning difference Type. This is
    /// not a projection of a difference into an absolute temperature point.
    pub const fn storage_coordinate(self) -> ExactDecimalQuantity {
        self.coordinate
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        crate::semantic_digest(
            EXACT_TEMPERATURE_DIFFERENCE_INFO_ID,
            &self.coordinate.encode(),
        )
    }

    pub fn compare(self, other: Self) -> Result<Ordering, QuantityConversionRefusal> {
        super::wide_conversion::compare_differences(self.coordinate, other.coordinate)
    }

    pub fn convert_to_target<'a>(
        self,
        target: ResolvedQuantitySuffix<'a>,
    ) -> Result<ExactTemperatureDifferenceTargetCoordinate<'a>, QuantityConversionRefusal> {
        let (unit, exponent) = super::target::target_parts(target);
        if unit.dimension() != QuantityDimension::Temperature {
            return Err(QuantityConversionRefusal::IncompatibleDimensions);
        }
        let coordinate =
            super::wide_conversion::difference_to_target_decimal(self.coordinate, unit, exponent)?;
        Ok(ExactTemperatureDifferenceTargetCoordinate {
            target,
            coefficient: coordinate.coefficient(),
            exponent: coordinate.exponent(),
        })
    }

    pub const fn transform(self) -> (i128, i128, i128) {
        let (scale, _, denominator) = self.coordinate.unit().canonical_transform();
        (scale, 0, denominator)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactTemperatureDifferenceTargetCoordinate<'a> {
    target: ResolvedQuantitySuffix<'a>,
    coefficient: i128,
    exponent: i16,
}

impl<'a> ExactTemperatureDifferenceTargetCoordinate<'a> {
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactTemperatureDifferenceConversionReceipt<'a> {
    original: &'a str,
    source: ExactTemperatureDifference,
    source_suffix: ResolvedQuantitySuffix<'a>,
    target: ResolvedQuantitySuffix<'a>,
    result: Result<ExactTemperatureDifferenceTargetCoordinate<'a>, QuantityConversionRefusal>,
}

impl<'a> ExactTemperatureDifferenceConversionReceipt<'a> {
    pub fn check(
        original: &'a str,
        target: &'a str,
    ) -> Result<Self, ExactQuantityConversionRequestRefusal> {
        let source = ExactTemperatureDifference::parse_plot_literal(original)
            .map_err(ExactQuantityConversionRequestRefusal::TemperatureDifferenceSource)?;
        let (source_suffix, target) =
            super::receipt::resolve_conversion_suffixes(original, target)?;
        Ok(Self {
            original,
            source,
            source_suffix,
            target,
            result: source.convert_to_target(target),
        })
    }
    pub const fn original(self) -> &'a str {
        self.original
    }
    pub const fn source(self) -> ExactTemperatureDifference {
        self.source
    }
    pub const fn source_suffix(self) -> ResolvedQuantitySuffix<'a> {
        self.source_suffix
    }
    pub const fn target(self) -> ResolvedQuantitySuffix<'a> {
        self.target
    }
    pub const fn result(
        self,
    ) -> Result<ExactTemperatureDifferenceTargetCoordinate<'a>, QuantityConversionRefusal> {
        self.result
    }
    pub const fn source_transform(self) -> (i128, i128, i128) {
        self.source.transform()
    }
    pub fn target_transform(self) -> (i128, i128, i128) {
        let (unit, _) = super::target::target_parts(self.target);
        let (scale, _, denominator) = unit.canonical_transform();
        (scale, 0, denominator)
    }
}
