//! Shared exact rational quantity law with fixed admitted arithmetic storage.

use super::QuantityConversionRefusal;
use core::cmp::Ordering;

pub(super) trait Arithmetic {
    type Number: Copy;
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

    pub(super) fn multiply(self, other: Self) -> Result<Self, QuantityConversionRefusal> {
        Ok(Self::new(
            A::multiply(self.numerator, other.numerator)
                .ok_or(QuantityConversionRefusal::Overflow)?,
            A::multiply(self.denominator, other.denominator)
                .ok_or(QuantityConversionRefusal::Overflow)?,
        ))
    }
    pub(super) fn divide(self, other: Self) -> Result<Self, QuantityConversionRefusal> {
        Ok(Self::new(
            A::multiply(self.numerator, other.denominator)
                .ok_or(QuantityConversionRefusal::Overflow)?,
            A::multiply(self.denominator, other.numerator)
                .ok_or(QuantityConversionRefusal::Overflow)?,
        ))
    }
    pub(super) fn add(self, other: Self) -> Result<Self, QuantityConversionRefusal> {
        let a = A::multiply(self.numerator, other.denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        let b = A::multiply(other.numerator, self.denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        Ok(Self::new(
            A::add(a, b).ok_or(QuantityConversionRefusal::Overflow)?,
            A::multiply(self.denominator, other.denominator)
                .ok_or(QuantityConversionRefusal::Overflow)?,
        ))
    }
    pub(super) fn negate(self) -> Result<Self, QuantityConversionRefusal> {
        Ok(Self::new(
            A::negate(self.numerator).ok_or(QuantityConversionRefusal::Overflow)?,
            self.denominator,
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
