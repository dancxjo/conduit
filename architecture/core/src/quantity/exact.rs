//! One canonical bounded decimal Quantity representation.
//!
//! Numeric domain projections require explicit checked integer admission.
//! This codec admits exact decimal coordinates, not arbitrary rational output
//! or an implicit rounding policy. Target admission remains a separate contract.

use crate::{QuantitySuffixRefusal, ResolvedQuantitySuffix};
use crate::{Unit, UnitRefusal};

pub const QUANTITY_INFO_ID: &str = "value/quantity@1";
pub const QUANTITY_ENCODED_LEN: usize = 22;
pub const QUANTITY_MAX_SIGNIFICANT_DIGITS: usize = 38;
pub const QUANTITY_MAX_NUMBER_BYTES: usize = 96;
pub const QUANTITY_MAX_LITERAL_BYTES: usize = 128;
pub const QUANTITY_MAX_EXPONENT: i16 = 128;
const MAX_COEFFICIENT: i128 = 10_i128.pow(38) - 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quantity {
    coefficient: i128,
    exponent: i16,
    unit: Unit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityRefusal {
    LiteralTooLong,
    NumberTooLong,
    InvalidNumber,
    UnsupportedExponentNotation,
    SignificantDigitsExceeded,
    ExponentOutOfRange,
    Unit(QuantitySuffixRefusal),
    WrongEncodingLength,
    UnsupportedEncodingVersion(u8),
    InvalidUnit(UnitRefusal),
    NonCanonicalEncoding,
}

impl Quantity {
    /// Integer construction is exact; general decimal construction is checked.
    pub const fn new(value: i64, unit: Unit) -> Self {
        let mut coefficient = value as i128;
        let mut exponent = 0;
        if coefficient != 0 {
            while coefficient % 10 == 0 {
                coefficient /= 10;
                exponent += 1;
            }
        }
        Self {
            coefficient,
            exponent,
            unit,
        }
    }

    /// Bounded canonical authored evidence. Values without an admitted literal
    /// spelling refuse rather than inventing unsupported exponent notation.
    pub fn canonical_literal(self) -> Result<alloc::string::String, QuantityRefusal> {
        use alloc::string::ToString;
        let digits = self.coefficient.unsigned_abs().to_string();
        let negative = self.coefficient < 0;
        let mut number = alloc::string::String::new();
        if negative {
            number.push('-');
        }
        if self.exponent >= 0 {
            number.push_str(&digits);
            for _ in 0..self.exponent {
                number.push('0');
            }
        } else {
            let places = self.exponent.unsigned_abs() as usize;
            if places < digits.len() {
                let position = digits.len() - places;
                number.push_str(&digits[..position]);
                number.push('.');
                number.push_str(&digits[position..]);
            } else {
                number.push_str("0.");
                for _ in 0..places - digits.len() {
                    number.push('0');
                }
                number.push_str(&digits);
            }
        }
        if number.len() > QUANTITY_MAX_NUMBER_BYTES {
            return Err(QuantityRefusal::NumberTooLong);
        }
        number.push_str(&self.unit.canonical_symbol());
        if number.len() > QUANTITY_MAX_LITERAL_BYTES {
            return Err(QuantityRefusal::LiteralTooLong);
        }
        Ok(number)
    }

    /// The coordinate is `coefficient * 10^exponent` in the reviewed unit.
    /// Every field is checked before admission; normalization is bounded and
    /// never changes the reviewed unit or materializes a large power of ten.
    pub fn from_decimal(
        mut coefficient: i128,
        mut exponent: i16,
        unit: Unit,
    ) -> Result<Self, QuantityRefusal> {
        if !(-QUANTITY_MAX_EXPONENT..=QUANTITY_MAX_EXPONENT).contains(&exponent) {
            return Err(QuantityRefusal::ExponentOutOfRange);
        }
        if coefficient == 0 {
            exponent = 0;
        } else {
            while coefficient % 10 == 0 && exponent < QUANTITY_MAX_EXPONENT {
                coefficient /= 10;
                exponent += 1;
            }
        }
        if !(-MAX_COEFFICIENT..=MAX_COEFFICIENT).contains(&coefficient) {
            return Err(QuantityRefusal::SignificantDigitsExceeded);
        }
        Ok(Self {
            coefficient,
            exponent,
            unit,
        })
    }

    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }
    pub const fn exponent(self) -> i16 {
        self.exponent
    }
    pub const fn unit(self) -> Unit {
        self.unit
    }

    pub const fn dimension(self) -> super::QuantityDimension {
        self.unit.dimension()
    }

    /// Reviewed base reference transform. The complete equation is
    /// `(coefficient * 10^(exponent + unit.decimal_exponent()) * scale + offset) / denominator`.
    /// Unit prefix exponents never multiply an affine offset.
    pub const fn reference_transform(self) -> (i128, i128, i128) {
        self.unit.canonical_transform()
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        crate::semantic_digest(QUANTITY_INFO_ID, &self.encode())
    }

    pub fn parse_plot_literal(literal: &str) -> Result<Self, QuantityRefusal> {
        if literal.len() > QUANTITY_MAX_LITERAL_BYTES {
            return Err(QuantityRefusal::LiteralTooLong);
        }
        let value_end = literal
            .char_indices()
            .find_map(|(index, character)| {
                (!(character.is_ascii_digit()
                    || character == '.'
                    || (index == 0 && character == '-')))
                    .then_some(index)
            })
            .unwrap_or(literal.len());
        let (number, suffix) = literal.split_at(value_end);
        if number.is_empty() || number == "-" {
            return Err(QuantityRefusal::InvalidNumber);
        }
        if number.len() > QUANTITY_MAX_NUMBER_BYTES {
            return Err(QuantityRefusal::NumberTooLong);
        }
        // Scientific exponent syntax is not a second interpretation of a unit
        // suffix. In particular, `Em` remains the reviewed exa-meter spelling.
        if matches!(suffix.as_bytes().first(), Some(b'e' | b'E'))
            && suffix
                .as_bytes()
                .get(1)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'+' || *byte == b'-')
        {
            return Err(QuantityRefusal::UnsupportedExponentNotation);
        }
        let resolved = ResolvedQuantitySuffix::resolve(suffix).map_err(QuantityRefusal::Unit)?;
        let unit = Unit::from_resolved(resolved);
        let prefix_exponent = 0;
        let magnitude = number.strip_prefix('-').unwrap_or(number);
        let (whole, fraction) = match magnitude.split_once('.') {
            Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
            Some(_) => return Err(QuantityRefusal::InvalidNumber),
            None => (magnitude, ""),
        };
        if whole.is_empty()
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(QuantityRefusal::InvalidNumber);
        }
        let digits = || whole.bytes().chain(fraction.bytes());
        let last_nonzero = digits()
            .enumerate()
            .filter(|(_, byte)| *byte != b'0')
            .map(|(index, _)| index)
            .last();
        let Some(last_nonzero) = last_nonzero else {
            return Self::from_decimal(0, 0, unit);
        };
        let trailing_zeroes = whole.len() + fraction.len() - last_nonzero - 1;
        let mut coefficient = 0_i128;
        let mut significant_digits = 0;
        for byte in digits().take(last_nonzero + 1) {
            if coefficient != 0 || byte != b'0' {
                significant_digits += 1;
                if significant_digits > QUANTITY_MAX_SIGNIFICANT_DIGITS {
                    return Err(QuantityRefusal::SignificantDigitsExceeded);
                }
            }
            coefficient = coefficient * 10 + i128::from(byte - b'0');
        }
        if number.starts_with('-') {
            coefficient = -coefficient;
        }
        // The input bounds make these lengths exactly representable by i16.
        let mut exponent = prefix_exponent - fraction.len() as i16 + trailing_zeroes as i16;
        // A literal's folded scale is not an encoded exponent field. Use the
        // remaining coefficient capacity before refusing its exact coordinate.
        // At most 38 iterations fit; the next multiplication refuses.
        while exponent > QUANTITY_MAX_EXPONENT {
            coefficient = coefficient
                .checked_mul(10)
                .filter(|value| (-MAX_COEFFICIENT..=MAX_COEFFICIENT).contains(value))
                .ok_or(QuantityRefusal::SignificantDigitsExceeded)?;
            exponent -= 1;
        }
        Self::from_decimal(coefficient, exponent, unit)
    }

    /// Admit an integer coordinate for a selected domain realization, refusing
    /// fractional precision and signed range overflow explicitly.
    pub fn to_i64(self, target: Unit) -> Result<i64, super::QuantityConversionRefusal> {
        super::wide_conversion::to_i64(self, target)
    }

    pub fn convert(self, target: Unit) -> Result<Self, super::QuantityConversionRefusal> {
        self.convert_to_decimal(target)
    }

    /// Explicit conversion into the bounded exact decimal target profile.
    /// A non-terminating decimal refuses rather than rounding.
    pub fn convert_to_decimal(
        self,
        target: Unit,
    ) -> Result<Self, super::QuantityConversionRefusal> {
        super::wide_conversion::to_decimal(self, target)
    }

    /// Explicit unsigned integer projection in the requested reviewed unit.
    /// Uses the same exact conversion law, then checks integer precision and
    /// the full unsigned range. No negative value or fraction is rounded.
    pub fn convert_to_u64(self, target: Unit) -> Result<u64, super::QuantityConversionRefusal> {
        use super::QuantityConversionRefusal as R;
        let converted = self.convert_to_decimal(target)?;
        if converted.coefficient < 0 {
            return Err(R::Overflow);
        }
        if converted.exponent < 0 {
            return Err(R::Inexact);
        }
        let coefficient = u64::try_from(converted.coefficient).map_err(|_| R::Overflow)?;
        let scale = 10_u64
            .checked_pow(converted.exponent as u32)
            .ok_or(R::Overflow)?;
        coefficient.checked_mul(scale).ok_or(R::Overflow)
    }

    /// Compare physical values in a common exact rational reference, without
    /// selecting a lossy unit or increasing the fixed arithmetic capacity.
    pub fn compare(
        self,
        other: Self,
    ) -> Result<core::cmp::Ordering, super::QuantityConversionRefusal> {
        super::wide_conversion::compare(self, other)
    }

    pub fn encode(self) -> [u8; QUANTITY_ENCODED_LEN] {
        let mut bytes = [0; QUANTITY_ENCODED_LEN];
        bytes[0] = 1;
        bytes[1..4].copy_from_slice(&self.unit.encode());
        bytes[4..6].copy_from_slice(&self.exponent.to_le_bytes());
        bytes[6..].copy_from_slice(&self.coefficient.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, QuantityRefusal> {
        if bytes.len() != QUANTITY_ENCODED_LEN {
            return Err(QuantityRefusal::WrongEncodingLength);
        }
        if bytes[0] != 1 {
            return Err(QuantityRefusal::UnsupportedEncodingVersion(bytes[0]));
        }
        let unit = Unit::decode(&bytes[1..4]).map_err(QuantityRefusal::InvalidUnit)?;
        let exponent = i16::from_le_bytes(bytes[4..6].try_into().unwrap());
        let coefficient = i128::from_le_bytes(bytes[6..].try_into().unwrap());
        let quantity = Self::from_decimal(coefficient, exponent, unit)?;
        if quantity.encode() != bytes {
            return Err(QuantityRefusal::NonCanonicalEncoding);
        }
        Ok(quantity)
    }
}

impl serde::Serialize for Quantity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.encode(), serializer)
    }
}
impl<'de> serde::Deserialize<'de> for Quantity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = <[u8; QUANTITY_ENCODED_LEN] as serde::Deserialize>::deserialize(deserializer)?;
        Self::decode(&bytes).map_err(|_| serde::de::Error::custom("invalid canonical Quantity"))
    }
}
