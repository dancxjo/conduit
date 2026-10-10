//! Shared exact rational quantity law with fixed admitted arithmetic storage.

use super::{CatalogUnit, QuantityConversionRefusal};
use core::cmp::Ordering;

pub(super) trait Arithmetic {
    type Number: Copy;
    fn integer(value: i128) -> Self::Number;
    fn add(left: Self::Number, right: Self::Number) -> Option<Self::Number>;
    fn multiply(left: Self::Number, right: Self::Number) -> Option<Self::Number>;
    fn negate(value: Self::Number) -> Option<Self::Number>;
    fn compare(left: Self::Number, right: Self::Number) -> Ordering;
    fn exact_i64(
        numerator: Self::Number,
        denominator: Self::Number,
    ) -> Result<i64, QuantityConversionRefusal>;
}

pub(super) struct Fraction<A: Arithmetic> {
    numerator: A::Number,
    denominator: A::Number,
}

impl<A: Arithmetic> Fraction<A> {
    pub(super) fn new(numerator: A::Number, denominator: A::Number) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub(super) fn parts(self) -> (A::Number, A::Number) {
        (self.numerator, self.denominator)
    }

    pub(super) fn into_canonical(
        self,
        source: CatalogUnit,
    ) -> Result<Self, QuantityConversionRefusal> {
        self.with_source_transform(source.canonical_transform())
    }

    pub(super) fn with_source_transform(
        self,
        (scale, offset, denominator): (i128, i128, i128),
    ) -> Result<Self, QuantityConversionRefusal> {
        let scaled = A::multiply(self.numerator, A::integer(scale))
            .ok_or(QuantityConversionRefusal::Overflow)?;
        let offset = A::multiply(A::integer(offset), self.denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        Ok(Self::new(
            A::add(scaled, offset).ok_or(QuantityConversionRefusal::Overflow)?,
            A::multiply(self.denominator, A::integer(denominator))
                .ok_or(QuantityConversionRefusal::Overflow)?,
        ))
    }

    pub(super) fn with_target_transform(
        self,
        (scale, offset, denominator): (i128, i128, i128),
    ) -> Result<Self, QuantityConversionRefusal> {
        let scaled = A::multiply(self.numerator, A::integer(denominator))
            .ok_or(QuantityConversionRefusal::Overflow)?;
        let offset = A::multiply(A::integer(offset), self.denominator)
            .and_then(A::negate)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        Ok(Self::new(
            A::add(scaled, offset).ok_or(QuantityConversionRefusal::Overflow)?,
            A::multiply(self.denominator, A::integer(scale))
                .ok_or(QuantityConversionRefusal::Overflow)?,
        ))
    }

    pub(super) fn integer(self) -> Result<i64, QuantityConversionRefusal> {
        A::exact_i64(self.numerator, self.denominator)
    }

    pub(super) fn compare(self, other: Self) -> Result<Ordering, QuantityConversionRefusal> {
        let left = A::multiply(self.numerator, other.denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        let right = A::multiply(other.numerator, self.denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        Ok(A::compare(left, right))
    }
}
