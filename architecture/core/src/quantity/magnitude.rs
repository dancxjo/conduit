//! Fixed storage for exact quantity conversion intermediates.
//!
//! 576 decimal digits cover the checked profile's 38-digit coordinate, ±128
//! exponent, reviewed rational unit transforms and pairwise cross products.
//! Arithmetic never allocates; every loop has a fixed capacity bound.

use core::cmp::Ordering;

const LIMBS: usize = 64;
const RADIX: u64 = 1_000_000_000;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) struct Magnitude([u32; LIMBS]);

impl Magnitude {
    pub(super) const ZERO: Self = Self([0; LIMBS]);

    pub(super) fn from_u128(mut value: u128) -> Self {
        let mut limbs = [0; LIMBS];
        for limb in &mut limbs {
            *limb = (value % u128::from(RADIX)) as u32;
            value /= u128::from(RADIX);
            if value == 0 {
                break;
            }
        }
        Self(limbs)
    }

    pub(super) fn power_of_ten(exponent: u16) -> Option<Self> {
        let limb = usize::from(exponent / 9);
        if limb >= LIMBS {
            return None;
        }
        let mut limbs = [0; LIMBS];
        limbs[limb] = 10_u32.pow(u32::from(exponent % 9));
        Some(Self(limbs))
    }

    pub(super) fn cmp(self, other: Self) -> Ordering {
        for index in (0..LIMBS).rev() {
            match self.0[index].cmp(&other.0[index]) {
                Ordering::Equal => {}
                order => return order,
            }
        }
        Ordering::Equal
    }

    pub(super) fn checked_add(self, other: Self) -> Option<Self> {
        let mut result = [0; LIMBS];
        let mut carry = 0;
        for (index, limb) in result.iter_mut().enumerate() {
            let sum = u64::from(self.0[index]) + u64::from(other.0[index]) + carry;
            *limb = (sum % RADIX) as u32;
            carry = sum / RADIX;
        }
        (carry == 0).then_some(Self(result))
    }

    pub(super) fn checked_sub(self, other: Self) -> Option<Self> {
        if self.cmp(other) == Ordering::Less {
            return None;
        }
        let mut result = [0; LIMBS];
        let mut borrow = 0_i64;
        for (index, limb) in result.iter_mut().enumerate() {
            let difference = i64::from(self.0[index]) - i64::from(other.0[index]) - borrow;
            borrow = i64::from(difference < 0);
            *limb = (difference + borrow * RADIX as i64) as u32;
        }
        Some(Self(result))
    }

    pub(super) fn checked_mul(self, other: Self) -> Option<Self> {
        let mut result = [0; LIMBS];
        for index in 0..LIMBS {
            if self.0[index] == 0 {
                continue;
            }
            if other.0[LIMBS - index..].iter().any(|limb| *limb != 0) {
                return None;
            }
            let mut carry = 0;
            for offset in 0..LIMBS - index {
                let position = index + offset;
                // Maximum is RADIX² - 1, below u64::MAX.
                let sum = u64::from(self.0[index]) * u64::from(other.0[offset])
                    + u64::from(result[position])
                    + carry;
                result[position] = (sum % RADIX) as u32;
                carry = sum / RADIX;
            }
            if carry != 0 {
                return None;
            }
        }
        Some(Self(result))
    }

    pub(super) fn divide_small(self, divisor: u32) -> Option<(Self, u32)> {
        if divisor == 0 {
            return None;
        }
        let mut result = [0; LIMBS];
        let mut remainder = 0_u64;
        for index in (0..LIMBS).rev() {
            let dividend = remainder * RADIX + u64::from(self.0[index]);
            result[index] = (dividend / u64::from(divisor)) as u32;
            remainder = dividend % u64::from(divisor);
        }
        Some((Self(result), remainder as u32))
    }

    pub(super) fn checked_mul_small(self, multiplier: u32) -> Option<Self> {
        let mut result = [0; LIMBS];
        let mut carry = 0_u64;
        for (index, limb) in result.iter_mut().enumerate() {
            let product = u64::from(self.0[index]) * u64::from(multiplier) + carry;
            *limb = (product % RADIX) as u32;
            carry = product / RADIX;
        }
        (carry == 0).then_some(Self(result))
    }

    /// Binary long-division remainder with a fixed 1920-bit work/storage bound.
    pub(super) fn remainder(self, denominator: Self) -> Option<Self> {
        if denominator == Self::ZERO {
            return None;
        }
        if self.cmp(denominator) == Ordering::Less {
            return Some(self);
        }
        let mut bits = [false; LIMBS * 30];
        let mut count = 0;
        let mut value = self;
        while value != Self::ZERO {
            let (quotient, remainder) = value.divide_small(2)?;
            *bits.get_mut(count)? = remainder != 0;
            count += 1;
            value = quotient;
        }
        let mut remainder = Self::ZERO;
        for bit in bits[..count].iter().rev() {
            remainder = remainder.checked_mul_small(2)?;
            if *bit {
                remainder = remainder.checked_add(Self::from_u128(1))?;
            }
            if remainder.cmp(denominator) != Ordering::Less {
                remainder = remainder.checked_sub(denominator)?;
            }
        }
        Some(remainder)
    }

    /// Find a bounded integer quotient without constructing an unbounded value.
    /// At most 64 bisection steps are needed for any u64 limit.
    pub(super) fn exact_quotient(self, denominator: Self, limit: u64) -> Option<u64> {
        if denominator == Self::ZERO {
            return None;
        }
        let mut lower = 0;
        let mut upper = limit;
        while lower <= upper {
            let middle = lower + (upper - lower) / 2;
            let product = denominator.checked_mul(Self::from_u128(u128::from(middle)))?;
            match product.cmp(self) {
                Ordering::Equal => return Some(middle),
                Ordering::Less => {
                    if middle == limit {
                        break;
                    }
                    lower = middle + 1;
                }
                Ordering::Greater => {
                    if middle == 0 {
                        break;
                    }
                    upper = middle - 1;
                }
            }
        }
        None
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) struct SignedMagnitude {
    negative: bool,
    magnitude: Magnitude,
}

impl SignedMagnitude {
    pub(super) fn from_i128(value: i128) -> Self {
        Self {
            negative: value < 0,
            magnitude: Magnitude::from_u128(value.unsigned_abs()),
        }
    }

    pub(super) fn negative(self) -> bool {
        self.negative
    }
    pub(super) fn magnitude(self) -> Magnitude {
        self.magnitude
    }

    pub(super) fn negated(self) -> Self {
        Self {
            negative: !self.negative && self.magnitude != Magnitude::ZERO,
            ..self
        }
    }

    pub(super) fn checked_mul(self, other: Magnitude) -> Option<Self> {
        let magnitude = self.magnitude.checked_mul(other)?;
        Some(Self {
            negative: self.negative && magnitude != Magnitude::ZERO,
            magnitude,
        })
    }

    pub(super) fn checked_product(self, other: Self) -> Option<Self> {
        let magnitude = self.magnitude.checked_mul(other.magnitude)?;
        Some(Self {
            negative: self.negative != other.negative && magnitude != Magnitude::ZERO,
            magnitude,
        })
    }

    pub(super) fn checked_add(self, other: Self) -> Option<Self> {
        let (negative, magnitude) = if self.negative == other.negative {
            (self.negative, self.magnitude.checked_add(other.magnitude)?)
        } else if self.magnitude.cmp(other.magnitude) == Ordering::Less {
            (other.negative, other.magnitude.checked_sub(self.magnitude)?)
        } else {
            (self.negative, self.magnitude.checked_sub(other.magnitude)?)
        };
        Some(Self {
            negative: negative && magnitude != Magnitude::ZERO,
            magnitude,
        })
    }

    pub(super) fn cmp(self, other: Self) -> Ordering {
        match (self.negative, other.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => self.magnitude.cmp(other.magnitude),
            (true, true) => other.magnitude.cmp(self.magnitude),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limb_arithmetic_matches_independent_native_integer_cases() {
        for left in [0, 1, 999_999_999, 1_000_000_000, u64::MAX as u128] {
            for right in [0, 1, 999_999_999, 1_000_000_001, u64::MAX as u128] {
                let a = Magnitude::from_u128(left);
                let b = Magnitude::from_u128(right);
                assert_eq!(a.checked_add(b), Some(Magnitude::from_u128(left + right)));
                assert_eq!(a.checked_mul(b), Some(Magnitude::from_u128(left * right)));
                assert_eq!(a.cmp(b), left.cmp(&right));
                assert_eq!(
                    a.checked_sub(b),
                    left.checked_sub(right).map(Magnitude::from_u128)
                );
            }
        }
    }

    #[test]
    fn fixed_capacity_preserves_carries_and_refuses_overflow() {
        let largest_power = Magnitude::power_of_ten(575).unwrap();
        assert_eq!(Magnitude::power_of_ten(576), None);
        assert_eq!(largest_power.checked_mul(Magnitude::from_u128(10)), None);
        let preceding = largest_power.checked_sub(Magnitude::from_u128(1)).unwrap();
        assert_eq!(
            preceding.checked_add(Magnitude::from_u128(1)),
            Some(largest_power)
        );
        assert_eq!(
            largest_power.checked_mul(Magnitude::ZERO),
            Some(Magnitude::ZERO)
        );
    }

    #[test]
    fn bounded_quotients_distinguish_integrality_and_sign_normalization() {
        let denominator = Magnitude::power_of_ten(128).unwrap();
        let numerator = denominator
            .checked_mul(Magnitude::from_u128(i64::MAX as u128))
            .unwrap();
        assert_eq!(
            numerator.exact_quotient(denominator, i64::MAX as u64),
            Some(i64::MAX as u64)
        );
        assert_eq!(
            numerator
                .checked_sub(Magnitude::from_u128(1))
                .unwrap()
                .exact_quotient(denominator, i64::MAX as u64),
            None
        );
        assert_eq!(Magnitude::ZERO.exact_quotient(Magnitude::ZERO, 1), None);
        let zero = SignedMagnitude::from_i128(-9)
            .checked_add(SignedMagnitude::from_i128(9))
            .unwrap();
        assert!(!zero.negative());
        assert_eq!(zero.magnitude(), Magnitude::ZERO);
        assert_eq!(zero.negated(), zero);
        assert_eq!(
            SignedMagnitude::from_i128(-9).cmp(SignedMagnitude::from_i128(-8)),
            Ordering::Less
        );
        assert_eq!(
            SignedMagnitude::from_i128(-9).checked_mul(Magnitude::from_u128(2)),
            Some(SignedMagnitude::from_i128(-18))
        );
    }
}
