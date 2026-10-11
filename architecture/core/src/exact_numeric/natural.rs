//! Fixed scratch/storage for the admitted exact numeric extent.
use super::{ExactNumericRefusal as R, ExactNumericWork as Work, EXACT_NUMERIC_MAX_BITS};
use core::cmp::Ordering;
const LIMBS: usize = EXACT_NUMERIC_MAX_BITS / 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Natural(pub(super) [u32; LIMBS]);
impl Natural {
    pub(super) const ZERO: Self = Self([0; LIMBS]);
    pub(super) const ONE: Self = {
        let mut limbs = [0; LIMBS];
        limbs[0] = 1;
        Self(limbs)
    };
    pub(super) fn from_u128(mut value: u128) -> Self {
        let mut result = Self::ZERO;
        for limb in result.0.iter_mut().take(4) {
            *limb = value as u32;
            value >>= 32;
        }
        result
    }
    pub(super) fn bits(self) -> usize {
        self.0
            .iter()
            .rposition(|limb| *limb != 0)
            .map_or(0, |index| {
                index * 32 + (32 - self.0[index].leading_zeros() as usize)
            })
    }
    pub(super) fn cmp(self, other: Self) -> Ordering {
        for (left, right) in self.0.iter().zip(other.0.iter()).rev() {
            match left.cmp(right) {
                Ordering::Equal => {}
                order => return order,
            }
        }
        Ordering::Equal
    }
    pub(super) fn add(self, other: Self, work: &mut Work) -> Result<Self, R> {
        work.charge(LIMBS as u64)?;
        let mut result = Self::ZERO;
        let mut carry = 0_u64;
        for (index, limb) in result.0.iter_mut().enumerate() {
            let value = u64::from(self.0[index]) + u64::from(other.0[index]) + carry;
            *limb = value as u32;
            carry = value >> 32;
        }
        if carry != 0 {
            Err(R::ExtentExceeded)
        } else {
            Ok(result)
        }
    }
    pub(super) fn sub(self, other: Self, work: &mut Work) -> Result<Self, R> {
        work.charge(LIMBS as u64)?;
        let mut result = Self::ZERO;
        let mut borrow = 0_i64;
        for (index, limb) in result.0.iter_mut().enumerate() {
            let value = i64::from(self.0[index]) - i64::from(other.0[index]) - borrow;
            *limb = value as u32;
            borrow = i64::from(value < 0);
        }
        if borrow != 0 {
            Err(R::OutOfRange)
        } else {
            Ok(result)
        }
    }
    pub(super) fn mul(self, other: Self, work: &mut Work) -> Result<Self, R> {
        work.charge((2 * LIMBS * LIMBS + LIMBS) as u64)?;
        let mut result = Self::ZERO;
        for index in 0..LIMBS {
            if self.0[index] == 0 {
                continue;
            }
            if other.0[LIMBS - index..].iter().any(|limb| *limb != 0) {
                return Err(R::ExtentExceeded);
            }
            let mut carry = 0_u64;
            for offset in 0..LIMBS - index {
                let position = index + offset;
                let value = u64::from(self.0[index]) * u64::from(other.0[offset])
                    + u64::from(result.0[position])
                    + carry;
                result.0[position] = value as u32;
                carry = value >> 32;
            }
            if carry != 0 {
                return Err(R::ExtentExceeded);
            }
        }
        Ok(result)
    }
    pub(super) fn small(self, factor: u32, add: u32, work: &mut Work) -> Result<Self, R> {
        work.charge(LIMBS as u64)?;
        let mut result = Self::ZERO;
        let mut carry = u64::from(add);
        for (index, limb) in result.0.iter_mut().enumerate() {
            let value = u64::from(self.0[index]) * u64::from(factor) + carry;
            *limb = value as u32;
            carry = value >> 32;
        }
        if carry != 0 {
            Err(R::ExtentExceeded)
        } else {
            Ok(result)
        }
    }
    fn shifted(self, shift: usize, work: &mut Work) -> Result<Self, R> {
        work.charge(LIMBS as u64)?;
        if self.bits() + shift > EXACT_NUMERIC_MAX_BITS {
            return Err(R::ExtentExceeded);
        }
        let mut result = Self::ZERO;
        let whole = shift / 32;
        let part = shift % 32;
        for index in 0..LIMBS - whole {
            result.0[index + whole] |= self.0[index] << part;
            if part != 0 && index + whole + 1 < LIMBS {
                result.0[index + whole + 1] |= self.0[index] >> (32 - part);
            }
        }
        Ok(result)
    }
    pub(super) fn divide(self, divisor: Self, work: &mut Work) -> Result<(Self, Self), R> {
        work.charge((4 * LIMBS) as u64)?;
        if divisor == Self::ZERO {
            return Err(R::DivisionByZero);
        }
        if divisor == Self::ONE {
            return Ok((self, Self::ZERO));
        }
        let mut quotient = Self::ZERO;
        let mut remainder = self;
        if self.cmp(divisor) == Ordering::Less {
            return Ok((quotient, remainder));
        }
        for shift in (0..=self.bits() - divisor.bits()).rev() {
            let shifted = divisor.shifted(shift, work)?;
            work.charge(LIMBS as u64)?;
            if remainder.cmp(shifted) != Ordering::Less {
                remainder = remainder.sub(shifted, work)?;
                quotient.0[shift / 32] |= 1_u32 << (shift % 32);
            }
        }
        Ok((quotient, remainder))
    }
    pub(super) fn gcd(mut self, mut other: Self, work: &mut Work) -> Result<Self, R> {
        while other != Self::ZERO {
            let remainder = self.divide(other, work)?.1;
            self = other;
            other = remainder;
        }
        Ok(self)
    }
    pub(super) fn exact_divide(self, divisor: Self, work: &mut Work) -> Result<Self, R> {
        let (quotient, remainder) = self.divide(divisor, work)?;
        if remainder != Self::ZERO {
            return Err(R::NonCanonicalEncoding);
        }
        Ok(quotient)
    }
    pub(super) fn to_u128(self) -> Result<u128, R> {
        if self.0[4..].iter().any(|limb| *limb != 0) {
            return Err(R::OutOfRange);
        }
        Ok(self.0[..4]
            .iter()
            .rev()
            .fold(0_u128, |value, limb| (value << 32) | u128::from(*limb)))
    }
    pub(super) fn read(bytes: &[u8]) -> Result<Self, R> {
        if bytes.len() > LIMBS * 4 {
            return Err(R::ExtentExceeded);
        }
        if bytes.last() == Some(&0) {
            return Err(R::NonCanonicalEncoding);
        }
        let mut result = Self::ZERO;
        for (index, byte) in bytes.iter().enumerate() {
            result.0[index / 4] |= u32::from(*byte) << (index % 4 * 8);
        }
        Ok(result)
    }
    pub(super) fn write(self, bytes: &mut [u8]) -> usize {
        let length = self.bits().div_ceil(8);
        for (index, byte) in bytes.iter_mut().take(length).enumerate() {
            *byte = (self.0[index / 4] >> (index % 4 * 8)) as u8;
        }
        length
    }
}
