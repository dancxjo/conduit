use super::{NumericRefusal, integer::Magnitude};
use core::cmp::Ordering;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactRational {
    pub(super) negative: bool,
    pub(super) numerator: Magnitude,
    pub(super) denominator: Magnitude,
}
impl ExactRational {
    pub fn from_i64(n: i64) -> Self {
        Self {
            negative: n < 0,
            numerator: Magnitude::from_u64(n.unsigned_abs()),
            denominator: Magnitude::from_u64(1),
        }
    }
    pub(super) fn new(
        negative: bool,
        numerator: Magnitude,
        denominator: Magnitude,
    ) -> Result<Self, NumericRefusal> {
        if denominator.is_zero() {
            return Err(NumericRefusal::DivisionByZero);
        }
        if numerator.is_zero() {
            return Ok(Self::from_i64(0));
        }
        let gcd = Magnitude::gcd(numerator.clone(), denominator.clone())?;
        let numerator = numerator.exact_div(&gcd)?;
        let denominator = denominator.exact_div(&gcd)?;
        numerator.admit()?;
        denominator.admit()?;
        Ok(Self {
            negative,
            numerator,
            denominator,
        })
    }
    /// Exact dyadic coordinate, useful for inspected IEEE realization boundaries.
    pub fn dyadic(coefficient: u64, exponent: i16) -> Result<Self, NumericRefusal> {
        if exponent.unsigned_abs() > 4096 {
            return Err(NumericRefusal::WorkBudget);
        }
        let mut power = Magnitude::from_u64(1);
        for _ in 0..exponent.unsigned_abs() {
            power = power.mul_small(2)?;
        }
        if exponent >= 0 {
            Self::new(
                false,
                Magnitude::from_u64(coefficient).mul(&power)?,
                Magnitude::from_u64(1),
            )
        } else {
            Self::new(false, Magnitude::from_u64(coefficient), power)
        }
    }
    pub fn integer(digits: &str) -> Result<Self, NumericRefusal> {
        let (negative, digits) = signed(digits);
        Self::new(negative, Magnitude::parse(digits)?, Magnitude::from_u64(1))
    }
    pub fn ratio(numerator: &str, denominator: &str) -> Result<Self, NumericRefusal> {
        let (a, n) = signed(numerator);
        let (b, d) = signed(denominator);
        Self::new(a != b, Magnitude::parse(n)?, Magnitude::parse(d)?)
    }
    pub fn decimal(source: &str) -> Result<Self, NumericRefusal> {
        if source.len() > 2304 {
            return Err(NumericRefusal::StorageBudget);
        }
        let (negative, source) = signed(source);
        let (mantissa, exp) = match source.find(['e', 'E']) {
            Some(at) => (
                &source[..at],
                source[at + 1..]
                    .parse::<i32>()
                    .map_err(|_| NumericRefusal::InvalidNumber)?,
            ),
            None => (source, 0),
        };
        let mut digits = alloc::string::String::new();
        let mut after = None;
        for c in mantissa.chars() {
            if c == '.' {
                if after.is_some() {
                    return Err(NumericRefusal::InvalidNumber);
                }
                after = Some(0i32);
            } else if c.is_ascii_digit() {
                digits.push(c);
                if let Some(n) = after.as_mut() {
                    *n += 1;
                }
            } else {
                return Err(NumericRefusal::InvalidNumber);
            }
        }
        let exponent = exp
            .checked_sub(after.unwrap_or(0))
            .ok_or(NumericRefusal::IntegerBudget)?;
        let n = Magnitude::parse(&digits)?;
        if n.is_zero() {
            return Ok(Self::from_i64(0));
        }
        if exponent >= 0 {
            Self::new(
                negative,
                n.mul(&Magnitude::power10(exponent as usize)?)?,
                Magnitude::from_u64(1),
            )
        } else {
            Self::new(
                negative,
                n,
                Magnitude::power10(exponent.unsigned_abs() as usize)?,
            )
        }
    }
    pub fn checked_i64(&self) -> Result<i64, crate::NumericProjectionRefusal> {
        if !self.denominator.is_one() {
            return Err(crate::NumericProjectionRefusal::Inexact);
        }
        self.as_i64()
            .ok_or(crate::NumericProjectionRefusal::Overflow)
    }
    pub fn checked_u64(&self) -> Result<u64, crate::NumericProjectionRefusal> {
        if !self.denominator.is_one() {
            return Err(crate::NumericProjectionRefusal::Inexact);
        }
        if self.negative {
            return Err(crate::NumericProjectionRefusal::Overflow);
        }
        self.numerator
            .to_u64()
            .ok_or(crate::NumericProjectionRefusal::Overflow)
    }
    pub fn as_i64(&self) -> Option<i64> {
        if !self.denominator.is_one() {
            return None;
        }
        let n = self.numerator.to_u64()?;
        if self.negative {
            if n == 1u64 << 63 {
                Some(i64::MIN)
            } else {
                i64::try_from(n).ok().map(|n| -n)
            }
        } else {
            i64::try_from(n).ok()
        }
    }
    pub fn is_zero(&self) -> bool {
        self.numerator.is_zero()
    }
    pub fn is_negative(&self) -> bool {
        self.negative
    }
    pub fn negated(&self) -> Self {
        let mut n = self.clone();
        if !n.is_zero() {
            n.negative = !n.negative;
        }
        n
    }
    pub fn checked_add(&self, other: &Self) -> Result<Self, NumericRefusal> {
        let gcd = Magnitude::gcd(self.denominator.clone(), other.denominator.clone())?;
        let a = self.denominator.exact_div(&gcd)?;
        let b = other.denominator.exact_div(&gcd)?;
        let left = self.numerator.mul(&b)?;
        let right = other.numerator.mul(&a)?;
        let (negative, numerator) = if self.negative == other.negative {
            (self.negative, left.add(&right)?)
        } else {
            match left.compare(&right) {
                Ordering::Less => (other.negative, right.sub(&left)),
                _ => (self.negative, left.sub(&right)),
            }
        };
        Self::new(negative, numerator, a.mul(&other.denominator)?)
    }
    pub fn checked_mul(&self, other: &Self) -> Result<Self, NumericRefusal> {
        let g1 = Magnitude::gcd(self.numerator.clone(), other.denominator.clone())?;
        let g2 = Magnitude::gcd(other.numerator.clone(), self.denominator.clone())?;
        Self::new(
            self.negative != other.negative,
            self.numerator
                .exact_div(&g1)?
                .mul(&other.numerator.exact_div(&g2)?)?,
            self.denominator
                .exact_div(&g2)?
                .mul(&other.denominator.exact_div(&g1)?)?,
        )
    }
    pub fn checked_div(&self, other: &Self) -> Result<Self, NumericRefusal> {
        if other.is_zero() {
            return Err(NumericRefusal::DivisionByZero);
        }
        self.checked_mul(&Self {
            negative: other.negative,
            numerator: other.denominator.clone(),
            denominator: other.numerator.clone(),
        })
    }
    pub fn compare(&self, other: &Self) -> Result<Ordering, NumericRefusal> {
        if self.negative != other.negative {
            return Ok(if self.negative {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let ordering = self
            .numerator
            .mul(&other.denominator)?
            .compare(&other.numerator.mul(&self.denominator)?);
        Ok(if self.negative {
            ordering.reverse()
        } else {
            ordering
        })
    }
    pub(super) fn encode_into(&self, out: &mut alloc::vec::Vec<u8>) {
        out.push(u8::from(self.negative));
        self.numerator.encode_into(out);
        self.denominator.encode_into(out);
    }
    pub(super) fn decode(cursor: &mut super::codec::Cursor<'_>) -> Result<Self, NumericRefusal> {
        let negative = match cursor.byte()? {
            0 => false,
            1 => true,
            _ => return Err(NumericRefusal::NonCanonical),
        };
        let numerator = Magnitude::decode(cursor)?;
        let denominator = Magnitude::decode(cursor)?;
        if denominator.is_zero() || (numerator.is_zero() && (negative || !denominator.is_one())) {
            return Err(NumericRefusal::NonCanonical);
        }
        let gcd = Magnitude::gcd(numerator.clone(), denominator.clone())?;
        if !gcd.is_one() {
            return Err(NumericRefusal::NonCanonical);
        }
        Ok(Self {
            negative,
            numerator,
            denominator,
        })
    }
}
fn signed(source: &str) -> (bool, &str) {
    if let Some(s) = source.strip_prefix('-') {
        (true, s)
    } else {
        (false, source.strip_prefix('+').unwrap_or(source))
    }
}
