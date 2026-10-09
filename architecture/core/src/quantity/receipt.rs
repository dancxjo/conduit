//! Source-preserving exact conversion facts. Construction establishes both
//! descriptors from the pinned catalogue before attempting representation.

use super::{
    ExactDecimalQuantity, ExactDecimalQuantityRefusal, ExactQuantityTargetCoordinate,
    QuantityConversionRefusal, QuantityUnit,
};
use crate::{QuantitySuffixRefusal, ResolvedQuantitySuffix};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExactQuantityConversionRequestRefusal {
    Source(ExactDecimalQuantityRefusal),
    TemperatureDifferenceSource(super::ExactTemperatureDifferenceRefusal),
    Target(QuantitySuffixRefusal),
    TargetTooLong,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactQuantityConversionReceipt<'a> {
    original: &'a str,
    source: ExactDecimalQuantity,
    source_suffix: ResolvedQuantitySuffix<'a>,
    target: ResolvedQuantitySuffix<'a>,
    result: Result<ExactQuantityTargetCoordinate<'a>, QuantityConversionRefusal>,
}

impl<'a> ExactQuantityConversionReceipt<'a> {
    pub fn check(
        original: &'a str,
        target: &'a str,
    ) -> Result<Self, ExactQuantityConversionRequestRefusal> {
        let source = ExactDecimalQuantity::parse_plot_literal(original)
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
    pub const fn source(self) -> ExactDecimalQuantity {
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
    /// Exact physical reference equation `(coordinate * scale + offset) / denominator`.
    /// Decimal exponents belong to the source coordinate and target descriptor;
    /// they never multiply an affine offset.
    pub const fn source_transform(self) -> (i128, i128, i128) {
        self.source.unit().canonical_transform()
    }
    pub const fn target_transform(self) -> (i128, i128, i128) {
        let unit: QuantityUnit = match self.target.base() {
            Some(base) => base.unit(),
            None => self.target.legacy_unit().unwrap(),
        };
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
    if target.len() > super::EXACT_DECIMAL_MAX_LITERAL_BYTES {
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
