use super::*;
/// Exact declaration scalar; powers of ten stay symbolic until required for addition.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DefinitionScalar {
    pub numerator: i128,
    pub denominator: i128,
    pub decimal_exponent: i16,
}
impl DefinitionScalar {
    pub fn new(
        numerator: i128,
        denominator: i128,
        decimal_exponent: i16,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if denominator <= 0 {
            return Err(PhysicalDefinitionRefusal::InvalidTransform);
        }
        if !(-128..=128).contains(&decimal_exponent) {
            return Err(PhysicalDefinitionRefusal::InvalidExponent);
        }
        if numerator == 0 {
            return Ok(Self {
                numerator: 0,
                denominator: 1,
                decimal_exponent: 0,
            });
        }
        let gcd = scalar_gcd(numerator.unsigned_abs(), denominator as u128) as i128;
        let mut value = Self {
            numerator: numerator / gcd,
            denominator: denominator / gcd,
            decimal_exponent,
        };
        while value.numerator % 10 == 0 && value.decimal_exponent < 128 {
            value.numerator /= 10;
            value.decimal_exponent += 1;
        }
        while value.denominator % 10 == 0 && value.decimal_exponent > -128 {
            value.denominator /= 10;
            value.decimal_exponent -= 1;
        }
        Ok(value)
    }
    /// Canonical exact meaning, independent of decimal versus rational spelling.
    pub(super) fn identity_bytes(self) -> [u8; 41] {
        let mut bytes = [0; 41];
        if self.numerator == 0 {
            return bytes;
        }
        bytes[0] = if self.numerator < 0 { 2 } else { 1 };
        let gcd = scalar_gcd(self.numerator.unsigned_abs(), self.denominator as u128);
        let mut n = self.numerator.unsigned_abs() / gcd;
        let mut d = self.denominator as u128 / gcd;
        let mut twos = i32::from(self.decimal_exponent);
        let mut fives = twos;
        while n.is_multiple_of(2) {
            n /= 2;
            twos += 1;
        }
        while d.is_multiple_of(2) {
            d /= 2;
            twos -= 1;
        }
        while n.is_multiple_of(5) {
            n /= 5;
            fives += 1;
        }
        while d.is_multiple_of(5) {
            d /= 5;
            fives -= 1;
        }
        bytes[1..17].copy_from_slice(&n.to_le_bytes());
        bytes[17..33].copy_from_slice(&d.to_le_bytes());
        bytes[33..37].copy_from_slice(&twos.to_le_bytes());
        bytes[37..41].copy_from_slice(&fives.to_le_bytes());
        bytes
    }
    pub fn equivalent(self, other: Self) -> bool {
        self.equivalent_binary(0, other, 0)
    }
    pub fn equivalent_binary(self, left_binary: i16, other: Self, right_binary: i16) -> bool {
        let Ok(left) = Self::new(self.numerator, self.denominator, self.decimal_exponent) else {
            return false;
        };
        let Ok(right) = Self::new(other.numerator, other.denominator, other.decimal_exponent)
        else {
            return false;
        };
        let (self_value, other) = (left, right);
        if self_value.numerator == 0 || other.numerator == 0 {
            return self_value.numerator == other.numerator;
        }
        if (self_value.numerator < 0) != (other.numerator < 0) {
            return false;
        }
        fn facts(value: DefinitionScalar, binary: i16) -> (u128, u128, i32, i32) {
            let mut n = value.numerator.unsigned_abs();
            let mut d = value.denominator as u128;
            let mut twos = i32::from(value.decimal_exponent) + i32::from(binary);
            let mut fives = i32::from(value.decimal_exponent);
            while n.is_multiple_of(2) {
                n /= 2;
                twos += 1;
            }
            while d.is_multiple_of(2) {
                d /= 2;
                twos -= 1;
            }
            while n.is_multiple_of(5) {
                n /= 5;
                fives += 1;
            }
            while d.is_multiple_of(5) {
                d /= 5;
                fives -= 1;
            }
            (n, d, twos, fives)
        }
        // Admitted scalars are reduced; the residual fractions remain coprime.
        facts(self_value, left_binary) == facts(other, right_binary)
    }
    pub fn multiply(self, other: Self) -> Result<Self, PhysicalDefinitionRefusal> {
        let left_value = Self::new(self.numerator, self.denominator, self.decimal_exponent)?;
        let right_value = Self::new(other.numerator, other.denominator, other.decimal_exponent)?;
        let gcd1 = scalar_gcd(
            left_value.numerator.unsigned_abs(),
            right_value.denominator as u128,
        ) as i128;
        let gcd2 = scalar_gcd(
            right_value.numerator.unsigned_abs(),
            left_value.denominator as u128,
        ) as i128;
        let numerator = (left_value.numerator / gcd1)
            .checked_mul(right_value.numerator / gcd2)
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        let denominator = (left_value.denominator / gcd2)
            .checked_mul(right_value.denominator / gcd1)
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        let exponent = left_value
            .decimal_exponent
            .checked_add(right_value.decimal_exponent)
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        Self::rebalance(numerator, denominator, exponent)
    }
    fn rebalance(
        mut numerator: i128,
        mut denominator: i128,
        mut exponent: i16,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if numerator == 0 {
            return Self::new(0, 1, 0);
        }
        while exponent > 128 {
            numerator = numerator
                .checked_mul(10)
                .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
            exponent -= 1;
        }
        while exponent < -128 {
            denominator = denominator
                .checked_mul(10)
                .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
            exponent += 1;
        }
        Self::new(numerator, denominator, exponent)
    }
    /// Apply an exact binary factor after rational cancellation. When the
    /// numerator fills, a factor of two can remain symbolic as 10/5.
    pub fn multiply_binary(self, exponent: u16) -> Result<Self, PhysicalDefinitionRefusal> {
        let mut value = Self::new(self.numerator, self.denominator, self.decimal_exponent)?;
        if value.numerator == 0 {
            return Ok(value);
        }
        for _ in 0..exponent {
            if value.denominator % 2 == 0 {
                value.denominator /= 2;
            } else if let Some(numerator) = value.numerator.checked_mul(2) {
                value.numerator = numerator;
            } else if value.decimal_exponent < 128 {
                if value.numerator % 5 == 0 {
                    value.numerator /= 5;
                } else {
                    value.denominator = value
                        .denominator
                        .checked_mul(5)
                        .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
                }
                value.decimal_exponent += 1;
            } else {
                return Err(PhysicalDefinitionRefusal::TransformOverflow);
            }
        }
        Self::new(value.numerator, value.denominator, value.decimal_exponent)
    }
    pub fn checked_add(self, other: Self) -> Result<Self, PhysicalDefinitionRefusal> {
        let left_value = Self::new(self.numerator, self.denominator, self.decimal_exponent)?;
        let right_value = Self::new(other.numerator, other.denominator, other.decimal_exponent)?;
        if left_value.numerator == 0 {
            return Ok(right_value);
        }
        if right_value.numerator == 0 {
            return Ok(left_value);
        }
        // Exact cancellation does not need to materialize differently written
        // powers of ten merely to discover the zero result.
        let cancellation = if left_value.numerator > 0 && right_value.numerator < 0 {
            Self {
                numerator: -left_value.numerator,
                ..left_value
            }
            .equivalent(right_value)
        } else if right_value.numerator > 0 && left_value.numerator < 0 {
            left_value.equivalent(Self {
                numerator: -right_value.numerator,
                ..right_value
            })
        } else {
            false
        };
        if cancellation {
            return Self::new(0, 1, 0);
        }
        let exponent = left_value
            .decimal_exponent
            .min(right_value.decimal_exponent);
        let left_power = 10_i128
            .checked_pow((left_value.decimal_exponent - exponent) as u32)
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        let right_power = 10_i128
            .checked_pow((right_value.decimal_exponent - exponent) as u32)
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        let gcd = scalar_gcd(
            left_value.denominator as u128,
            right_value.denominator as u128,
        ) as i128;
        let left = left_value
            .numerator
            .checked_mul(right_value.denominator / gcd)
            .and_then(|n| n.checked_mul(left_power))
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        let right = right_value
            .numerator
            .checked_mul(left_value.denominator / gcd)
            .and_then(|n| n.checked_mul(right_power))
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        Self::new(
            left.checked_add(right)
                .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?,
            (left_value.denominator / gcd)
                .checked_mul(right_value.denominator)
                .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?,
            exponent,
        )
    }
}
pub(super) fn scalar_gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
