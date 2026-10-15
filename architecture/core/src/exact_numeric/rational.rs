//! Reduced rational denotation with positive denominator and one canonical zero.
use super::{
    natural::Natural, ExactNumericRefusal as R, ExactNumericWork as Work,
    EXACT_NUMERIC_MAX_DECIMAL_BYTES, EXACT_RATIONAL_MAX_ENCODED_BYTES,
};
use core::cmp::Ordering;

mod binary64;
mod decimal;

pub const EXACT_RATIONAL_ID: &str = "numeric/exact-rational@1";

/// Fixed storage, with no heap or symbolic interpreter on existing native paths.
/// Arithmetic intermediates must also fit the admitted 4096-bit extent. A larger
/// expression requires another admitted profile; extent refusal is never rounding.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ExactRational {
    negative: bool,
    numerator: Natural,
    denominator: Natural,
}
impl ExactRational {
    pub const ZERO: Self = Self {
        negative: false,
        numerator: Natural::ZERO,
        denominator: Natural::ONE,
    };
    pub const ONE: Self = Self {
        negative: false,
        numerator: Natural::ONE,
        denominator: Natural::ONE,
    };
    pub fn from_i128(value: i128) -> Self {
        Self {
            negative: value < 0,
            numerator: Natural::from_u128(value.unsigned_abs()),
            denominator: Natural::ONE,
        }
    }
    pub fn from_u128(value: u128) -> Self {
        Self {
            negative: false,
            numerator: Natural::from_u128(value),
            denominator: Natural::ONE,
        }
    }
    pub(super) fn integer_magnitude(self, work: &mut Work) -> Result<(u128, bool), R> {
        // Charge even short division paths; report inspection never repeats it.
        work.charge((super::EXACT_NUMERIC_MAX_BITS / 32) as u64)?;
        let (quotient, remainder) = self.numerator.divide(self.denominator, work)?;
        Ok((quotient.to_u128()?, remainder != Natural::ZERO))
    }
    pub fn ratio(numerator: i128, denominator: i128, work: &mut Work) -> Result<Self, R> {
        Self::normalized(
            numerator.is_negative() != denominator.is_negative(),
            Natural::from_u128(numerator.unsigned_abs()),
            Natural::from_u128(denominator.unsigned_abs()),
            work,
        )
    }
    /// Exact decimal parts, independent of authored spelling or physical Unit.
    pub fn from_decimal_parts(
        coefficient: i128,
        exponent: i16,
        work: &mut Work,
    ) -> Result<Self, R> {
        Self::from_i128(coefficient)
            .checked_multiply(Self::from_i128(10).checked_power(exponent, work)?, work)
    }
    /// Bounded exponentiation by squaring. Negative powers require a nonzero
    /// base; every intermediate obeys the same storage and work admission.
    /// The zero power is the multiplicative identity, including for a zero base.
    pub fn checked_power(self, exponent: i16, work: &mut Work) -> Result<Self, R> {
        let mut base = if exponent < 0 {
            Self::ONE.checked_divide(self, work)?
        } else {
            self
        };
        let mut remaining = exponent.unsigned_abs();
        let mut result = Self::ONE;
        while remaining != 0 {
            if remaining & 1 != 0 {
                result = result.checked_multiply(base, work)?;
            }
            remaining >>= 1;
            if remaining != 0 {
                base = base.checked_multiply(base, work)?;
            }
        }
        Ok(result)
    }
    fn normalized(
        negative: bool,
        numerator: Natural,
        denominator: Natural,
        work: &mut Work,
    ) -> Result<Self, R> {
        if denominator == Natural::ZERO {
            return Err(R::DivisionByZero);
        }
        if numerator == Natural::ZERO {
            return Ok(Self::ZERO);
        }
        let gcd = numerator.gcd(denominator, work)?;
        Ok(Self {
            negative,
            numerator: numerator.exact_divide(gcd, work)?,
            denominator: denominator.exact_divide(gcd, work)?,
        })
    }
    /// Decimal source spelling denotes an exact rational power of ten. No
    /// exponent syntax, truncation or binary float intermediate is introduced.
    pub fn parse_decimal(source: &str, work: &mut Work) -> Result<Self, R> {
        if source.len() > EXACT_NUMERIC_MAX_DECIMAL_BYTES {
            return Err(R::ExtentExceeded);
        }
        let negative = source.starts_with('-');
        let digits = source.strip_prefix('-').unwrap_or(source);
        if digits.is_empty() {
            return Err(R::InvalidLiteral);
        }
        let mut numerator = Natural::ZERO;
        let mut denominator = Natural::ONE;
        let mut decimal = false;
        let mut fraction = 0_usize;
        for (index, byte) in digits.bytes().enumerate() {
            if byte == b'.' {
                if decimal || index == 0 || index + 1 == digits.len() {
                    return Err(R::InvalidLiteral);
                }
                decimal = true;
                continue;
            }
            if !byte.is_ascii_digit() {
                return Err(R::InvalidLiteral);
            }
            numerator = numerator.small(10, u32::from(byte - b'0'), work)?;
            if decimal {
                fraction += 1;
            }
        }
        for _ in 0..fraction {
            denominator = denominator.small(10, 0, work)?;
        }
        Self::normalized(negative, numerator, denominator, work)
    }
    pub const fn is_negative(self) -> bool {
        self.negative
    }
    pub fn is_integer(self) -> bool {
        self.denominator == Natural::ONE
    }
    pub fn checked_u128(self) -> Result<u128, R> {
        if !self.is_integer() {
            return Err(R::InexactProjection);
        }
        if self.negative {
            return Err(R::OutOfRange);
        }
        self.numerator.to_u128()
    }
    /// Project a nonnegative exact coordinate into a bounded unsigned fraction.
    /// Neither component is rounded, saturated or replaced; wide or negative
    /// coordinates refuse before consumers adopt a smaller representation.
    pub fn checked_unsigned_ratio(self) -> Result<(u128, u128), R> {
        if self.negative {
            return Err(R::OutOfRange);
        }
        Ok((self.numerator.to_u128()?, self.denominator.to_u128()?))
    }
    pub fn checked_i128(self) -> Result<i128, R> {
        if !self.is_integer() {
            return Err(R::InexactProjection);
        }
        let magnitude = self.numerator.to_u128()?;
        if self.negative && magnitude == 1_u128 << 127 {
            return Ok(i128::MIN);
        }
        let value = i128::try_from(magnitude).map_err(|_| R::OutOfRange)?;
        Ok(if self.negative { -value } else { value })
    }
    pub fn negated(mut self) -> Self {
        if self.numerator != Natural::ZERO {
            self.negative = !self.negative;
        }
        self
    }
    pub fn checked_add(self, other: Self, work: &mut Work) -> Result<Self, R> {
        let gcd = self.denominator.gcd(other.denominator, work)?;
        let left_factor = other.denominator.exact_divide(gcd, work)?;
        let right_factor = self.denominator.exact_divide(gcd, work)?;
        let left = self.numerator.mul(left_factor, work)?;
        let right = other.numerator.mul(right_factor, work)?;
        let (negative, numerator) = if self.negative == other.negative {
            (self.negative, left.add(right, work)?)
        } else if left.cmp(right) == Ordering::Less {
            (other.negative, right.sub(left, work)?)
        } else {
            (self.negative, left.sub(right, work)?)
        };
        Self::normalized(
            negative,
            numerator,
            self.denominator.mul(left_factor, work)?,
            work,
        )
    }
    pub fn checked_subtract(self, other: Self, work: &mut Work) -> Result<Self, R> {
        self.checked_add(other.negated(), work)
    }
    pub fn checked_multiply(self, other: Self, work: &mut Work) -> Result<Self, R> {
        // Cancel across factors before multiplication, rather than imposing the
        // extent on a needlessly unreduced product.
        let left_gcd = self.numerator.gcd(other.denominator, work)?;
        let right_gcd = other.numerator.gcd(self.denominator, work)?;
        let numerator = self
            .numerator
            .exact_divide(left_gcd, work)?
            .mul(other.numerator.exact_divide(right_gcd, work)?, work)?;
        let denominator = self
            .denominator
            .exact_divide(right_gcd, work)?
            .mul(other.denominator.exact_divide(left_gcd, work)?, work)?;
        Self::normalized(
            self.negative != other.negative,
            numerator,
            denominator,
            work,
        )
    }
    pub fn checked_divide(self, other: Self, work: &mut Work) -> Result<Self, R> {
        if other.numerator == Natural::ZERO {
            return Err(R::DivisionByZero);
        }
        self.checked_multiply(
            Self {
                negative: other.negative,
                numerator: other.denominator,
                denominator: other.numerator,
            },
            work,
        )
    }
    /// Continued-fraction comparison avoids oversized cross-product scratch.
    pub fn checked_compare(self, other: Self, work: &mut Work) -> Result<Ordering, R> {
        if self.negative != other.negative {
            return Ok(if self.negative {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let (mut a, mut b) = (self.numerator, self.denominator);
        let (mut c, mut d) = (other.numerator, other.denominator);
        let mut reverse = self.negative;
        loop {
            let (left, left_rem) = a.divide(b, work)?;
            let (right, right_rem) = c.divide(d, work)?;
            let order = left.cmp(right);
            if order != Ordering::Equal {
                return Ok(if reverse { order.reverse() } else { order });
            }
            if left_rem == Natural::ZERO || right_rem == Natural::ZERO {
                let order = left_rem.cmp(right_rem);
                return Ok(if reverse { order.reverse() } else { order });
            }
            (a, b, c, d) = (b, left_rem, d, right_rem);
            reverse = !reverse;
        }
    }
    /// Version + sign + two u16 byte lengths + minimal little-endian magnitudes.
    pub fn encode(self, output: &mut [u8]) -> Result<usize, R> {
        let n = self.numerator.bits().div_ceil(8);
        let d = self.denominator.bits().div_ceil(8);
        let length = 6 + n + d;
        if output.len() < length {
            return Err(R::EncodingBufferTooSmall);
        }
        output[0] = 1;
        output[1] = u8::from(self.negative);
        output[2..4].copy_from_slice(&(n as u16).to_le_bytes());
        output[4..6].copy_from_slice(&(d as u16).to_le_bytes());
        self.numerator.write(&mut output[6..6 + n]);
        self.denominator.write(&mut output[6 + n..length]);
        Ok(length)
    }
    pub fn decode(bytes: &[u8], work: &mut Work) -> Result<Self, R> {
        if bytes.len() < 6 || bytes.len() > EXACT_RATIONAL_MAX_ENCODED_BYTES {
            return Err(R::NonCanonicalEncoding);
        }
        if bytes[0] != 1 {
            return Err(R::UnsupportedVersion);
        }
        if bytes[1] > 1 {
            return Err(R::NonCanonicalEncoding);
        }
        let n = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
        let d = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
        if bytes.len() != 6 + n + d {
            return Err(R::NonCanonicalEncoding);
        }
        let raw = Self {
            negative: bytes[1] == 1,
            numerator: Natural::read(&bytes[6..6 + n])?,
            denominator: Natural::read(&bytes[6 + n..])?,
        };
        let normalized = Self::normalized(raw.negative, raw.numerator, raw.denominator, work)?;
        if raw != normalized {
            return Err(R::NonCanonicalEncoding);
        }
        Ok(raw)
    }
    pub fn semantic_digest(self) -> [u8; 32] {
        let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let length = self
            .encode(&mut bytes)
            .expect("fixed rational codec extent");
        crate::semantic_digest(EXACT_RATIONAL_ID, &bytes[..length])
    }
}
