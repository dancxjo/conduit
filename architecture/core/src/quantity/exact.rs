//! Separately versioned bounded decimal quantity representation.
//!
//! Recognition of a unit is independent of legacy i64 storage eligibility.
//! This codec admits exact decimal coordinates, not arbitrary rational output
//! or an implicit rounding policy. Target admission remains a separate contract.

use super::{QuantityDecodeRefusal, QuantityUnit};
use crate::{QuantitySuffixRefusal, ResolvedQuantitySuffix};

pub const EXACT_DECIMAL_QUANTITY_INFO_ID: &str = "value/exact-decimal-quantity@1";
pub const EXACT_DECIMAL_QUANTITY_ENCODED_LEN: usize = 20;
pub const EXACT_DECIMAL_MAX_SIGNIFICANT_DIGITS: usize = 38;
pub const EXACT_DECIMAL_MAX_NUMBER_BYTES: usize = 96;
pub const EXACT_DECIMAL_MAX_LITERAL_BYTES: usize = 128;
pub const EXACT_DECIMAL_MAX_EXPONENT: i16 = 128;
const MAX_COEFFICIENT: i128 = 10_i128.pow(38) - 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExactDecimalQuantity {
    coefficient: i128,
    exponent: i16,
    unit: QuantityUnit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExactDecimalQuantityRefusal {
    LiteralTooLong,
    NumberTooLong,
    InvalidNumber,
    UnsupportedExponentNotation,
    SignificantDigitsExceeded,
    ExponentOutOfRange,
    Unit(QuantitySuffixRefusal),
    WrongEncodingLength,
    UnsupportedEncodingVersion(u8),
    InvalidUnit(QuantityDecodeRefusal),
    NonCanonicalEncoding,
}

impl ExactDecimalQuantity {
    /// The coordinate is `coefficient * 10^exponent` in the reviewed unit.
    /// Every field is checked before admission; normalization is bounded and
    /// never changes the reviewed unit or materializes a large power of ten.
    pub fn new(
        mut coefficient: i128,
        mut exponent: i16,
        unit: QuantityUnit,
    ) -> Result<Self, ExactDecimalQuantityRefusal> {
        if !(-EXACT_DECIMAL_MAX_EXPONENT..=EXACT_DECIMAL_MAX_EXPONENT).contains(&exponent) {
            return Err(ExactDecimalQuantityRefusal::ExponentOutOfRange);
        }
        if coefficient == 0 {
            exponent = 0;
        } else {
            while coefficient % 10 == 0 && exponent < EXACT_DECIMAL_MAX_EXPONENT {
                coefficient /= 10;
                exponent += 1;
            }
        }
        if !(-MAX_COEFFICIENT..=MAX_COEFFICIENT).contains(&coefficient) {
            return Err(ExactDecimalQuantityRefusal::SignificantDigitsExceeded);
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
    pub const fn unit(self) -> QuantityUnit {
        self.unit
    }

    pub const fn dimension(self) -> super::QuantityDimension {
        self.unit.dimension()
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        crate::semantic_digest(EXACT_DECIMAL_QUANTITY_INFO_ID, &self.encode())
    }

    pub fn parse_plot_literal(literal: &str) -> Result<Self, ExactDecimalQuantityRefusal> {
        if literal.len() > EXACT_DECIMAL_MAX_LITERAL_BYTES {
            return Err(ExactDecimalQuantityRefusal::LiteralTooLong);
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
            return Err(ExactDecimalQuantityRefusal::InvalidNumber);
        }
        if number.len() > EXACT_DECIMAL_MAX_NUMBER_BYTES {
            return Err(ExactDecimalQuantityRefusal::NumberTooLong);
        }
        // Scientific exponent syntax is not a second interpretation of a unit
        // suffix. In particular, `Em` remains the reviewed exa-meter spelling.
        if matches!(suffix.as_bytes().first(), Some(b'e' | b'E'))
            && suffix
                .as_bytes()
                .get(1)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'+' || *byte == b'-')
        {
            return Err(ExactDecimalQuantityRefusal::UnsupportedExponentNotation);
        }
        let resolved =
            ResolvedQuantitySuffix::resolve(suffix).map_err(ExactDecimalQuantityRefusal::Unit)?;
        let (unit, prefix_exponent) = match resolved.base() {
            Some(base) => (base.unit(), resolved.decimal_exponent().unwrap()),
            None => (resolved.legacy_unit().unwrap(), 0),
        };
        let magnitude = number.strip_prefix('-').unwrap_or(number);
        let (whole, fraction) = match magnitude.split_once('.') {
            Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
            Some(_) => return Err(ExactDecimalQuantityRefusal::InvalidNumber),
            None => (magnitude, ""),
        };
        if whole.is_empty()
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(ExactDecimalQuantityRefusal::InvalidNumber);
        }
        let digits = || whole.bytes().chain(fraction.bytes());
        let last_nonzero = digits()
            .enumerate()
            .filter(|(_, byte)| *byte != b'0')
            .map(|(index, _)| index)
            .last();
        let Some(last_nonzero) = last_nonzero else {
            return Self::new(0, 0, unit);
        };
        let trailing_zeroes = whole.len() + fraction.len() - last_nonzero - 1;
        let mut coefficient = 0_i128;
        let mut significant_digits = 0;
        for byte in digits().take(last_nonzero + 1) {
            if coefficient != 0 || byte != b'0' {
                significant_digits += 1;
                if significant_digits > EXACT_DECIMAL_MAX_SIGNIFICANT_DIGITS {
                    return Err(ExactDecimalQuantityRefusal::SignificantDigitsExceeded);
                }
            }
            coefficient = coefficient * 10 + i128::from(byte - b'0');
        }
        if number.starts_with('-') {
            coefficient = -coefficient;
        }
        // The input bounds make these lengths exactly representable by i16.
        let exponent = prefix_exponent - fraction.len() as i16 + trailing_zeroes as i16;
        Self::new(coefficient, exponent, unit)
    }

    /// Explicit projection to the legacy integer representation. Refuses
    /// inexactness, incompatible dimensions and range overflow without rounding.
    pub fn convert_to_legacy(
        self,
        target: QuantityUnit,
    ) -> Result<super::Quantity, super::QuantityConversionRefusal> {
        super::wide_conversion::to_legacy(self, target)
    }

    /// Compare physical values in a common exact rational reference, without
    /// selecting a lossy unit or increasing the legacy arithmetic profile.
    pub fn compare(
        self,
        other: Self,
    ) -> Result<core::cmp::Ordering, super::QuantityConversionRefusal> {
        super::wide_conversion::compare(self, other)
    }

    pub fn encode(self) -> [u8; EXACT_DECIMAL_QUANTITY_ENCODED_LEN] {
        let mut bytes = [0; EXACT_DECIMAL_QUANTITY_ENCODED_LEN];
        bytes[0] = 1;
        bytes[1] = self.unit.encode()[0];
        bytes[2..4].copy_from_slice(&self.exponent.to_le_bytes());
        bytes[4..].copy_from_slice(&self.coefficient.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ExactDecimalQuantityRefusal> {
        if bytes.len() != EXACT_DECIMAL_QUANTITY_ENCODED_LEN {
            return Err(ExactDecimalQuantityRefusal::WrongEncodingLength);
        }
        if bytes[0] != 1 {
            return Err(ExactDecimalQuantityRefusal::UnsupportedEncodingVersion(
                bytes[0],
            ));
        }
        let unit =
            QuantityUnit::decode(&bytes[1..2]).map_err(ExactDecimalQuantityRefusal::InvalidUnit)?;
        let exponent = i16::from_le_bytes(bytes[2..4].try_into().unwrap());
        let coefficient = i128::from_le_bytes(bytes[4..].try_into().unwrap());
        let quantity = Self::new(coefficient, exponent, unit)?;
        if quantity.encode() != bytes {
            return Err(ExactDecimalQuantityRefusal::NonCanonicalEncoding);
        }
        Ok(quantity)
    }
}
