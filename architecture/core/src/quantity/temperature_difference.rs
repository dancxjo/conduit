//! Explicit temperature differences share the bounded decimal coordinate law,
//! but never acquire an absolute point's offset or its semantic identity.
//! The Plot representation wraps the coordinate in a distinct checked record;
//! the point codec is not a standalone difference encoding.

use super::{
    ExactQuantityConversionRequestRefusal, Quantity, QuantityConversionRefusal, QuantityDimension,
    QuantityRefusal, Unit,
};
use crate::ResolvedQuantitySuffix;
use core::cmp::Ordering;

pub const EXACT_TEMPERATURE_DIFFERENCE_INFO_ID: &str = crate::BUILTIN_TEMPERATURE_DELTA_INFO_ID;
pub fn exact_temperature_difference_type() -> crate::StructuredInfoType {
    crate::StructuredInfoType::leaf(crate::kind_id(EXACT_TEMPERATURE_DIFFERENCE_INFO_ID))
        .expect("intrinsic declared temperature delta leaf")
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExactTemperatureDifferenceRefusal {
    Coordinate(QuantityRefusal),
    NotTemperature,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactTemperatureDifference {
    coordinate: Quantity,
}

impl ExactTemperatureDifference {
    pub fn new(
        coefficient: i128,
        exponent: i16,
        unit: Unit,
    ) -> Result<Self, ExactTemperatureDifferenceRefusal> {
        Self::admit(
            Quantity::from_decimal_role(coefficient, exponent, unit, crate::QuantityRole::Delta)
                .map_err(ExactTemperatureDifferenceRefusal::Coordinate)?,
        )
    }

    pub fn parse_plot_literal(literal: &str) -> Result<Self, ExactTemperatureDifferenceRefusal> {
        Self::admit(
            Quantity::parse_plot_literal(literal)
                .map_err(ExactTemperatureDifferenceRefusal::Coordinate)?,
        )
    }

    pub fn from_quantity(coordinate: Quantity) -> Result<Self, ExactTemperatureDifferenceRefusal> {
        Self::admit(coordinate)
    }

    fn admit(coordinate: Quantity) -> Result<Self, ExactTemperatureDifferenceRefusal> {
        if coordinate.family().identity() != crate::builtin_temperature_family().identity()
            || coordinate.role() != crate::QuantityRole::Delta
        {
            return Err(ExactTemperatureDifferenceRefusal::NotTemperature);
        }
        Ok(Self { coordinate })
    }

    /// Raw storage coordinates require the owning difference Type. This is
    /// not a projection of a difference into an absolute temperature point.
    pub const fn storage_coordinate(self) -> Quantity {
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

    pub fn convert_to_unit(
        self,
        target: crate::Unit,
    ) -> Result<(i128, i16), QuantityConversionRefusal> {
        if target.dimension() != QuantityDimension::Temperature {
            return Err(QuantityConversionRefusal::IncompatibleDimensions);
        }
        let coordinate =
            super::wide_conversion::difference_to_target_decimal(self.coordinate, target, 0)?;
        Ok((coordinate.coefficient(), coordinate.exponent()))
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
    pub fn from_checked(
        source: ExactTemperatureDifference,
        target_unit: crate::Unit,
        original: &'a str,
        target_source: &'a str,
    ) -> Result<Self, ExactQuantityConversionRequestRefusal> {
        if !source
            .storage_coordinate()
            .matches_literal_evidence(original)
        {
            return Err(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch);
        }
        let source_suffix =
            super::receipt::source_suffix_from_evidence(original, source.storage_coordinate())?;
        let target = ResolvedQuantitySuffix::from_unit(target_source, target_unit)
            .map_err(|_| ExactQuantityConversionRequestRefusal::TargetEvidenceMismatch)?;
        let result = source
            .convert_to_unit(target_unit)
            .map(
                |(coefficient, exponent)| ExactTemperatureDifferenceTargetCoordinate {
                    target,
                    coefficient,
                    exponent,
                },
            );
        Ok(Self {
            original,
            source,
            source_suffix,
            target,
            result,
        })
    }

    pub fn check(
        original: &'a str,
        target: &'a str,
    ) -> Result<Self, ExactQuantityConversionRequestRefusal> {
        let source = ExactTemperatureDifference::parse_plot_literal(original)
            .map_err(ExactQuantityConversionRequestRefusal::TemperatureDifferenceSource)?;
        let target_unit = Unit::resolve(target).map_err(|e| {
            ExactQuantityConversionRequestRefusal::Target(crate::QuantitySuffixRefusal::Unit(e))
        })?;
        Self::from_checked(source, target_unit, original, target)
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
}
