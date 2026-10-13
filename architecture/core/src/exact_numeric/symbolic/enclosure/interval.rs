//! Outward interval arithmetic over exact rational endpoints.
use super::{ExactNumericWork as Work, ExactRational as Q, MathematicalEnclosureRefusal as R};
use core::cmp::Ordering;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathematicalInterval {
    lower: Q,
    upper: Q,
}
impl MathematicalInterval {
    pub const fn lower(self) -> Q {
        self.lower
    }
    pub const fn upper(self) -> Q {
        self.upper
    }
    pub(super) const fn point(value: Q) -> Self {
        Self {
            lower: value,
            upper: value,
        }
    }
    pub(super) const fn bounds(lower: Q, upper: Q) -> Self {
        Self { lower, upper }
    }
    pub(super) fn width(self, work: &mut Work) -> Result<Q, R> {
        Ok(self.upper.checked_subtract(self.lower, work)?)
    }
    pub(super) fn negated(self) -> Self {
        Self::bounds(self.upper.negated(), self.lower.negated())
    }
    pub(super) fn add(self, other: Self, work: &mut Work) -> Result<Self, R> {
        Ok(Self::bounds(
            self.lower.checked_add(other.lower, work)?,
            self.upper.checked_add(other.upper, work)?,
        ))
    }
    pub(super) fn subtract(self, other: Self, work: &mut Work) -> Result<Self, R> {
        self.add(other.negated(), work)
    }
    pub(super) fn multiply(self, other: Self, work: &mut Work) -> Result<Self, R> {
        let products = [
            self.lower.checked_multiply(other.lower, work)?,
            self.lower.checked_multiply(other.upper, work)?,
            self.upper.checked_multiply(other.lower, work)?,
            self.upper.checked_multiply(other.upper, work)?,
        ];
        let mut lower = products[0];
        let mut upper = lower;
        for product in products.into_iter().skip(1) {
            if product.checked_compare(lower, work)? == Ordering::Less {
                lower = product;
            }
            if product.checked_compare(upper, work)? == Ordering::Greater {
                upper = product;
            }
        }
        Ok(Self::bounds(lower, upper))
    }
    pub(super) fn scale(self, factor: Q, work: &mut Work) -> Result<Self, R> {
        self.multiply(Self::point(factor), work)
    }
}
