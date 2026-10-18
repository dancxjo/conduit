//! Shared integer-only, single-round finite IEEE conversion. No native float arithmetic.
use super::{ExactRational, Natural, Work, R};
use crate::Binary64Rounding;
use core::cmp::Ordering;
#[derive(Clone, Copy)]
pub(super) struct Format {
    fraction: u32,
    exponent_bits: u32,
    bias: i16,
}
pub(super) const BINARY32: Format = Format {
    fraction: 23,
    exponent_bits: 8,
    bias: 127,
};
pub(super) const BINARY64: Format = Format {
    fraction: 52,
    exponent_bits: 11,
    bias: 1023,
};
impl Format {
    fn sign(self) -> u64 {
        1 << (self.fraction + self.exponent_bits)
    }
}
impl ExactRational {
    /// The exact mathematical denotation of finite IEEE bits. Both signed
    /// zeros denote rational zero; the original IEEE carrier retains their bits.
    pub(super) fn from_ieee(bits: u64, format: Format, work: &mut Work) -> Result<Self, R> {
        work.charge(1)?;
        let exponent_mask = (1 << format.exponent_bits) - 1;
        let exponent = ((bits >> format.fraction) & exponent_mask) as i16;
        if u64::try_from(exponent).unwrap() == exponent_mask {
            return Err(R::OutOfRange);
        }
        let fraction = bits & ((1 << format.fraction) - 1);
        let (significand, shift) = if exponent == 0 {
            (fraction, 1 - format.bias - format.fraction as i16)
        } else {
            (
                (1 << format.fraction) | fraction,
                exponent - format.bias - format.fraction as i16,
            )
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
        Self::normalized(bits & format.sign() != 0, numerator, denominator, work)
    }
    pub(super) fn round_ieee(
        self,
        format: Format,
        rounding: Binary64Rounding,
        work: &mut Work,
    ) -> Result<(u64, bool), R> {
        work.charge(1)?;
        if self.numerator == Natural::ZERO {
            return Ok((0, false));
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
        if exponent > format.bias {
            return Err(R::OutOfRange);
        }
        let subnormal = exponent < 1 - format.bias;
        let shift = if subnormal {
            format.bias + format.fraction as i16 - 1
        } else {
            format.fraction as i16 - exponent
        };
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
            if significand == 1 << (format.fraction + 1) {
                significand >>= 1;
                exponent += 1;
            }
            if exponent > format.bias {
                return Err(R::OutOfRange);
            }
            (((exponent + format.bias) as u64) << format.fraction)
                | (significand - (1 << format.fraction))
        };
        Ok((
            magnitude | if self.negative { format.sign() } else { 0 },
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
