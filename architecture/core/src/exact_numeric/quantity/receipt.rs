//! Source correlation precedes independent rational coordinate realization.
use super::{RationalQuantity, RationalQuantityRefusal};
use crate::{
    ExactNumericWork, ExactQuantityConversionRequestRefusal, Quantity, ResolvedQuantitySuffix, Unit,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RationalQuantityConversionReceipt<'a> {
    original: &'a str,
    source: Quantity,
    source_suffix: ResolvedQuantitySuffix<'a>,
    target: ResolvedQuantitySuffix<'a>,
    result: Result<RationalQuantity, RationalQuantityRefusal>,
}
impl<'a> RationalQuantityConversionReceipt<'a> {
    pub fn from_checked(
        source: Quantity,
        target_unit: Unit,
        original: &'a str,
        target_source: &'a str,
        work: &mut ExactNumericWork,
    ) -> Result<Self, ExactQuantityConversionRequestRefusal> {
        let (source_suffix, target) = crate::quantity::checked_conversion_evidence(
            source,
            target_unit,
            original,
            target_source,
        )?;
        let result = RationalQuantity::from_quantity(source, work)
            .and_then(|value| value.convert_to_unit(target_unit, work));
        Ok(Self {
            original,
            source,
            source_suffix,
            target,
            result,
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
    pub const fn result(self) -> Result<RationalQuantity, RationalQuantityRefusal> {
        self.result
    }
}
