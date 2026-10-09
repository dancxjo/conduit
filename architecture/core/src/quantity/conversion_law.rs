//! Shared exact rational quantity law, specialized by the selected finite
//! arithmetic profile. Legacy quantities keep their existing small profile;
//! the extended profile admits wider fixed storage explicitly.

use super::{Quantity, QuantityConversionRefusal, QuantityUnit};
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

    pub(super) fn rational(
        numerator: i128,
        denominator: i128,
    ) -> Result<Self, QuantityConversionRefusal> {
        if denominator <= 0 {
            return Err(QuantityConversionRefusal::Inexact);
        }
        Ok(Self::new(A::integer(numerator), A::integer(denominator)))
    }

    pub(super) fn into_canonical(
        self,
        source: QuantityUnit,
    ) -> Result<Self, QuantityConversionRefusal> {
        let (scale, offset, denominator) = source.canonical_transform();
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

    pub(super) fn in_target(self, target: QuantityUnit) -> Result<Self, QuantityConversionRefusal> {
        let (scale, offset, denominator) = target.canonical_transform();
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

    pub(super) fn legacy_integer(
        self,
        target: QuantityUnit,
    ) -> Result<Quantity, QuantityConversionRefusal> {
        A::exact_i64(self.numerator, self.denominator).map(|value| Quantity::new(value, target))
    }

    pub(super) fn compare(self, other: Self) -> Result<Ordering, QuantityConversionRefusal> {
        let left = A::multiply(self.numerator, other.denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        let right = A::multiply(other.numerator, self.denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        Ok(A::compare(left, right))
    }
}
