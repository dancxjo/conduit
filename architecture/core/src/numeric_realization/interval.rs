//! Rational enclosures: every endpoint follows an exact inequality proof.
use super::NumericRealizationWork;
use crate::{MathValue, NumericRefusal};
use core::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RationalInterval {
    lower: MathValue,
    upper: MathValue,
}
impl RationalInterval {
    pub fn new(lower: MathValue, upper: MathValue) -> Result<Self, NumericRefusal> {
        if lower.as_rational().is_none() || upper.as_rational().is_none() {
            return Err(NumericRefusal::IndeterminateDomain);
        }
        match lower.compare(&upper)? {
            Some(Ordering::Less | Ordering::Equal) => Ok(Self { lower, upper }),
            _ => Err(NumericRefusal::Domain),
        }
    }
    pub fn point(value: MathValue) -> Result<Self, NumericRefusal> {
        Self::new(value.clone(), value)
    }
    pub fn lower(&self) -> &MathValue {
        &self.lower
    }
    pub fn upper(&self) -> &MathValue {
        &self.upper
    }
    pub fn width(&self) -> Result<MathValue, NumericRefusal> {
        self.upper.checked_sub(&self.lower)
    }
    pub fn add(&self, rhs: &Self) -> Result<Self, NumericRefusal> {
        Self::new(
            self.lower.checked_add(&rhs.lower)?,
            self.upper.checked_add(&rhs.upper)?,
        )
    }
    pub fn negate(&self) -> Result<Self, NumericRefusal> {
        Self::new(self.upper.checked_neg()?, self.lower.checked_neg()?)
    }
    pub fn subtract(&self, rhs: &Self) -> Result<Self, NumericRefusal> {
        self.add(&rhs.negate()?)
    }
    pub fn multiply(&self, rhs: &Self) -> Result<Self, NumericRefusal> {
        let products = [
            self.lower.checked_mul(&rhs.lower)?,
            self.lower.checked_mul(&rhs.upper)?,
            self.upper.checked_mul(&rhs.lower)?,
            self.upper.checked_mul(&rhs.upper)?,
        ];
        let mut lower = products[0].clone();
        let mut upper = lower.clone();
        for value in &products[1..] {
            if value.compare(&lower)? == Some(Ordering::Less) {
                lower = value.clone();
            }
            if value.compare(&upper)? == Some(Ordering::Greater) {
                upper = value.clone();
            }
        }
        Self::new(lower, upper)
    }
    pub fn divide(&self, rhs: &Self) -> Result<Self, NumericRefusal> {
        let zero = MathValue::integer(0);
        if rhs.lower.compare(&zero)? != Some(Ordering::Greater)
            && rhs.upper.compare(&zero)? != Some(Ordering::Less)
        {
            return Err(NumericRefusal::Domain);
        }
        self.multiply(&Self::new(
            MathValue::ONE.checked_div(&rhs.upper)?,
            MathValue::ONE.checked_div(&rhs.lower)?,
        )?)
    }
}
/// Alternating arctan series at 1/q. The omitted term bounds the signed remainder.
fn atan_reciprocal(
    q: i64,
    terms: u16,
    work: &mut NumericRealizationWork,
) -> Result<RationalInterval, NumericRefusal> {
    if q < 2 || terms == 0 || terms > 128 {
        return Err(NumericRefusal::WorkBudget);
    }
    let x = MathValue::ONE.checked_div(&MathValue::integer(q))?;
    let square = x.checked_mul(&x)?;
    let mut power = x;
    let mut sum = MathValue::integer(0);
    for index in 0..terms {
        work.charge(1)?;
        let term = power.checked_div(&MathValue::integer(2 * i64::from(index) + 1))?;
        sum = if index % 2 == 0 {
            sum.checked_add(&term)?
        } else {
            sum.checked_sub(&term)?
        };
        power = power.checked_mul(&square)?;
    }
    let remainder = power.checked_div(&MathValue::integer(2 * i64::from(terms) + 1))?;
    if terms.is_multiple_of(2) {
        RationalInterval::new(sum.clone(), sum.checked_add(&remainder)?)
    } else {
        RationalInterval::new(sum.checked_sub(&remainder)?, sum)
    }
}
/// Machin's exact identity pi = 16 atan(1/5) - 4 atan(1/239).
pub fn enclose_pi(
    terms: u16,
    work: &mut NumericRealizationWork,
) -> Result<RationalInterval, NumericRefusal> {
    atan_reciprocal(5, terms, work)?
        .multiply(&RationalInterval::point(MathValue::integer(16))?)?
        .subtract(
            &atan_reciprocal(239, terms, work)?
                .multiply(&RationalInterval::point(MathValue::integer(4))?)?,
        )
}
/// Exact bisection preserves lower^2 <= input <= upper^2 at every step.
pub fn enclose_sqrt(
    value: &MathValue,
    iterations: u16,
    work: &mut NumericRealizationWork,
) -> Result<RationalInterval, NumericRefusal> {
    if value.as_rational().is_none() {
        return Err(NumericRefusal::IndeterminateDomain);
    }
    if value.compare(&MathValue::integer(0))? == Some(Ordering::Less) {
        return Err(NumericRefusal::Domain);
    }
    if iterations > 4096 {
        return Err(NumericRefusal::WorkBudget);
    }
    let mut lower = MathValue::integer(0);
    let mut upper = if value.compare(&MathValue::ONE)? == Some(Ordering::Greater) {
        value.clone()
    } else {
        MathValue::ONE
    };
    for _ in 0..iterations {
        work.charge(1)?;
        let midpoint = lower
            .checked_add(&upper)?
            .checked_div(&MathValue::integer(2))?;
        match midpoint.checked_mul(&midpoint)?.compare(value)? {
            Some(Ordering::Equal) => return RationalInterval::point(midpoint),
            Some(Ordering::Less) => lower = midpoint,
            Some(Ordering::Greater) => upper = midpoint,
            None => return Err(NumericRefusal::IndeterminateDomain),
        }
    }
    RationalInterval::new(lower, upper)
}

/// Taylor's theorem uses |sin^(k)(x)|, |cos^(k)(x)| <= 1 on
/// the whole real line. The rational remainder covers every x in the interval.
pub fn enclose_sin_cos_radians(
    argument: &RationalInterval,
    terms: u16,
    cosine: bool,
    work: &mut NumericRealizationWork,
) -> Result<RationalInterval, NumericRefusal> {
    if terms == 0 || terms > 32 {
        return Err(NumericRefusal::WorkBudget);
    }
    let abs = |value: &MathValue| -> Result<MathValue, NumericRefusal> {
        if value.compare(&MathValue::integer(0))? == Some(Ordering::Less) {
            value.checked_neg()
        } else {
            Ok(value.clone())
        }
    };
    let a = abs(argument.lower())?;
    let b = abs(argument.upper())?;
    let maximum = if a.compare(&b)? == Some(Ordering::Greater) {
        a
    } else {
        b
    };
    if maximum.compare(&MathValue::integer(8))? == Some(Ordering::Greater) {
        return Err(NumericRefusal::IndeterminateDomain);
    }
    let mut sum = RationalInterval::point(MathValue::integer(0))?;
    let mut power = RationalInterval::point(MathValue::ONE)?;
    let mut factorial = MathValue::ONE;
    // Include degrees through 2*terms (cos) or 2*terms-1 (sin).
    let degree = 2 * terms - u16::from(!cosine);
    for index in 0..=degree {
        work.charge(4)?;
        if index % 2 == u16::from(!cosine) {
            let term = power.divide(&RationalInterval::point(factorial.clone())?)?;
            sum = if (index / 2) % 2 == 0 {
                sum.add(&term)?
            } else {
                sum.subtract(&term)?
            };
        }
        power = power.multiply(argument)?;
        factorial = factorial.checked_mul(&MathValue::integer(i64::from(index) + 1))?;
    }
    work.charge(3)?;
    let error = maximum
        .checked_pow(i64::from(degree) + 1)?
        .checked_div(&factorial)?;
    RationalInterval::new(
        sum.lower.checked_sub(&error)?,
        sum.upper.checked_add(&error)?,
    )
}
