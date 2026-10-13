//! Integer-only, single-round binary64 conversion. No native float arithmetic.
use super::{ExactRational, Natural, Work, R};
use crate::{Binary64Rounding, IeeeF64};
use core::cmp::Ordering;
const SIGN: u64 = 1 << 63;
impl ExactRational {
    /// The exact mathematical denotation of finite IEEE bits. Both signed
    /// zeros denote rational zero; the original IEEE carrier retains their bits.
    pub fn from_binary64(value: IeeeF64, work: &mut Work) -> Result<Self, R> {
        work.charge(1)?;
        let bits = value.bits();
        let exponent = ((bits >> 52) & 0x7ff) as i16;
        if exponent == 0x7ff {
            return Err(R::OutOfRange);
        }
        let fraction = bits & ((1 << 52) - 1);
        let (significand, shift) = if exponent == 0 {
            (fraction, -1074)
        } else {
            ((1 << 52) | fraction, exponent - 1023 - 52)
        };
        if significand == 0 {
            return Ok(Self::ZERO);
        }
        let mut numerator = Natural::from_u128(significand as u128);
        let mut denominator = Natural::ONE;
        if shift >= 0 {
            numerator = numerator.shifted(shift as usize, work)?;
        } else {
            denominator = denominator.shifted((-shift) as usize, work)?;
        }
        Self::normalized(bits & SIGN != 0, numerator, denominator, work)
    }
    pub(in crate::exact_numeric) fn round_binary64(
        self,
        rounding: Binary64Rounding,
        work: &mut Work,
    ) -> Result<(IeeeF64, bool), R> {
        work.charge(1)?;
        if self.numerator == Natural::ZERO {
            return Ok((IeeeF64::from_bits(0), false));
        }
        let mut exponent = self.numerator.bits() as i16 - self.denominator.bits() as i16;
        let below = if exponent >= 0 {
            self.numerator
                .cmp(self.denominator.shifted(exponent as usize, work)?)
        } else {
            self.numerator
                .shifted((-exponent) as usize, work)?
                .cmp(self.denominator)
        };
        if below == Ordering::Less {
            exponent -= 1;
        }
        if exponent > 1023 {
            return Err(R::OutOfRange);
        }
        let subnormal = exponent < -1022;
        let shift = if subnormal { 1074 } else { 52 - exponent };
        let (mut significand, remainder, divisor) = self.scaled_quotient(shift, work)?;
        let inexact = remainder != Natural::ZERO;
        let upward = if inexact {
            match rounding {
                Binary64Rounding::NearestTiesEven => {
                    let half = remainder.cmp(divisor.sub(remainder, work)?);
                    half == Ordering::Greater || (half == Ordering::Equal && significand & 1 != 0)
                }
                Binary64Rounding::TowardZero => false,
                Binary64Rounding::TowardPositive => !self.negative,
                Binary64Rounding::TowardNegative => self.negative,
            }
        } else {
            false
        };
        if upward {
            significand += 1;
        }
        let magnitude = if subnormal {
            // Carry from the largest subnormal is the smallest normal encoding.
            significand
        } else {
            if significand == 1 << 53 {
                significand >>= 1;
                exponent += 1;
            }
            if exponent > 1023 {
                return Err(R::OutOfRange);
            }
            (((exponent + 1023) as u64) << 52) | (significand - (1 << 52))
        };
        Ok((
            IeeeF64::from_bits(magnitude | if self.negative { SIGN } else { 0 }),
            inexact,
        ))
    }
    fn scaled_quotient(self, shift: i16, work: &mut Work) -> Result<(u64, Natural, Natural), R> {
        if shift < 0 {
            let divisor = self.denominator.shifted((-shift) as usize, work)?;
            let (quotient, remainder) = self.numerator.divide(divisor, work)?;
            return Ok((
                u64::try_from(quotient.to_u128()?).map_err(|_| R::OutOfRange)?,
                remainder,
                divisor,
            ));
        }
        let divisor = self.denominator;
        let (quotient, mut remainder) = self.numerator.divide(divisor, work)?;
        let mut quotient = u64::try_from(quotient.to_u128()?).map_err(|_| R::OutOfRange)?;
        // Stream fractional bits without shifting a full-extent numerator past
        // its storage. Comparing r with d-r avoids an overflowing 2*r as well.
        for _ in 0..shift {
            work.charge((super::super::EXACT_NUMERIC_MAX_BITS / 32 + 1) as u64)?;
            let complement = divisor.sub(remainder, work)?;
            let bit = remainder.cmp(complement) != Ordering::Less;
            remainder = if bit {
                remainder.sub(complement, work)?
            } else {
                remainder.add(remainder, work)?
            };
            quotient = quotient
                .checked_mul(2)
                .and_then(|v| v.checked_add(u64::from(bit)))
                .ok_or(R::OutOfRange)?;
        }
        Ok((quotient, remainder, divisor))
    }
}
