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
        let evidence_source = Quantity::parse_plot_literal(original)
            .map_err(ExactQuantityConversionRequestRefusal::Source)?;
        if evidence_source != source {
            return Err(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch);
        }
        let (source_suffix, target) = resolve_conversion_suffixes(original, target_source)?;
        if crate::Unit::from_resolved(target) != target_unit {
            return Err(ExactQuantityConversionRequestRefusal::TargetEvidenceMismatch);
        }
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
        let (source_suffix, target) = resolve_conversion_suffixes(original, target)?;
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
    /// Base scale, affine offset and denominator for the source reference equation.
    /// The full equation is `(coefficient * 10^(exponent + unit.decimal_exponent())
    /// * scale + offset) / denominator`; see [`Quantity::reference_transform`].
    /// Decimal exponents never multiply an affine offset.
    pub const fn source_transform(self) -> (i128, i128, i128) {
        self.source.reference_transform()
    }
    pub fn target_transform(self) -> (i128, i128, i128) {
        let unit = Unit::from_resolved(self.target);
        unit.canonical_transform()
    }
}

pub(super) fn resolve_conversion_suffixes<'a>(
    original: &'a str,
    target: &'a str,
) -> Result<
    (ResolvedQuantitySuffix<'a>, ResolvedQuantitySuffix<'a>),
    ExactQuantityConversionRequestRefusal,
> {
    if target.len() > super::QUANTITY_MAX_LITERAL_BYTES {
        return Err(ExactQuantityConversionRequestRefusal::TargetTooLong);
    }
    let suffix_start = original
        .char_indices()
        .find_map(|(index, character)| {
            (!(character.is_ascii_digit() || character == '.' || (index == 0 && character == '-')))
                .then_some(index)
        })
        .expect("a parsed quantity contains its suffix");
    let source_suffix = ResolvedQuantitySuffix::resolve(&original[suffix_start..])
        .expect("the checked parser resolved this identical suffix");
    let target = ResolvedQuantitySuffix::resolve(target)
        .map_err(ExactQuantityConversionRequestRefusal::Target)?;
    Ok((source_suffix, target))
}
