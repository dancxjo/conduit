//! Source-preserving exact conversion facts. Construction establishes both
//! descriptors from the pinned catalogue before attempting representation.

use super::{
    ExactQuantityTargetCoordinate, Quantity, QuantityConversionRefusal, QuantityRefusal, Unit,
};
use crate::{QuantitySuffixRefusal, ResolvedQuantitySuffix};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExactQuantityConversionRequestRefusal {
    Source(QuantityRefusal),
    TemperatureDifferenceSource(super::ExactTemperatureDifferenceRefusal),
    Target(QuantitySuffixRefusal),
    TargetTooLong,
    SourceEvidenceMismatch,
    TargetEvidenceMismatch,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactQuantityConversionReceipt<'a> {
    original: &'a str,
    source: Quantity,
    source_suffix: ResolvedQuantitySuffix<'a>,
    target: ResolvedQuantitySuffix<'a>,
    result: Result<ExactQuantityTargetCoordinate<'a>, QuantityConversionRefusal>,
}

impl<'a> ExactQuantityConversionReceipt<'a> {
    /// Source bytes correlate checked facts; conversion uses the checked values.
    pub fn from_checked(
        source: Quantity,
        target_unit: crate::Unit,
        original: &'a str,
        target_source: &'a str,
    ) -> Result<Self, ExactQuantityConversionRequestRefusal> {
        if target_source.len() > crate::UNIT_MAX_SOURCE_BYTES {
            return Err(ExactQuantityConversionRequestRefusal::TargetTooLong);
        }
        if !source.matches_literal_evidence(original) {
            return Err(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch);
        }
        let source_suffix = source_suffix_from_evidence(original, source)?;
        let target = ResolvedQuantitySuffix::from_unit(target_source, target_unit)
            .map_err(|_| ExactQuantityConversionRequestRefusal::TargetEvidenceMismatch)?;
        let result = source
            .convert_to_unit(target_unit)
            .map(|(coefficient, exponent)| {
                super::ExactQuantityTargetCoordinate::from_checked_parts(
                    target,
                    coefficient,
                    exponent,
                )
            });
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
        let source = Quantity::parse_plot_literal(original)
            .map_err(ExactQuantityConversionRequestRefusal::Source)?;
        let target_unit = Unit::resolve(target).map_err(|e| {
            ExactQuantityConversionRequestRefusal::Target(QuantitySuffixRefusal::Unit(e))
        })?;
        Self::from_checked(source, target_unit, original, target)
    }
    pub const fn original(self) -> &'a str {
        self.original
    }
    pub const fn source(self) -> Quantity {
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
    ) -> Result<ExactQuantityTargetCoordinate<'a>, QuantityConversionRefusal> {
        self.result
    }
}

pub(super) fn source_suffix_from_evidence<'a>(
    original: &'a str,
    source: Quantity,
) -> Result<ResolvedQuantitySuffix<'a>, ExactQuantityConversionRequestRefusal> {
    let unit = source.unit();
    let symbol = unit.symbol();
    let text = if original.trim_end().ends_with(')') {
        let inner = original.trim_end().strip_suffix(')').unwrap();
        inner
            .rsplit_once(',')
            .map(|(_, s)| s.trim())
            .ok_or(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch)?
    } else {
        original
            .strip_suffix(symbol)
            .map(|_| &original[original.len() - symbol.len()..])
            .ok_or(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch)?
    };
    ResolvedQuantitySuffix::from_unit(text, source.unit())
        .map_err(|_| ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch)
}
