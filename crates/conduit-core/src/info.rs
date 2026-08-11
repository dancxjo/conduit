//! Portable value contracts carried by typed Ports and Cords.
//!
//! These types describe information only. They do not create Gears, runtime
//! state, implementations, placements, or scheduler identities.

use core::convert::TryFrom;
use sha2::{Digest, Sha256};

pub const BOOL_INFO_ID: &str = "value/bool@1";
pub const BOOL_ENCODED_LEN: usize = 1;
pub const SCALAR_INFO_ID: &str = "value/scalar@1";
pub const SCALAR_ENCODED_LEN: usize = 8;

const SEMANTIC_DIGEST_DOMAIN: &[u8] = b"conduit.info.semantic.v1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InfoDecodeError {
    WrongLength {
        expected: usize,
        actual: usize,
    },
    NonCanonicalBoolean(u8),
    OutOfRange {
        field: &'static str,
        minimum: i64,
        maximum: i64,
        actual: i64,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScalarArithmeticError {
    Overflow,
}

/// The exact two-state value carried by `value/bool@1`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InfoBool(bool);

impl InfoBool {
    pub const FALSE: Self = Self(false);
    pub const TRUE: Self = Self(true);

    pub const fn new(value: bool) -> Self {
        Self(value)
    }

    pub const fn get(self) -> bool {
        self.0
    }

    /// Encodes false as `00` and true as `01`.
    pub const fn encode(self) -> [u8; BOOL_ENCODED_LEN] {
        [self.0 as u8]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        if encoded.len() != BOOL_ENCODED_LEN {
            return Err(InfoDecodeError::WrongLength {
                expected: BOOL_ENCODED_LEN,
                actual: encoded.len(),
            });
        }
        match encoded[0] {
            0 => Ok(Self::FALSE),
            1 => Ok(Self::TRUE),
            other => Err(InfoDecodeError::NonCanonicalBoolean(other)),
        }
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(BOOL_INFO_ID, &self.encode())
    }
}

/// A signed fixed-point scalar with exactly six decimal places.
///
/// The stored `i64` is a count of millionths. Every state is finite, semantic
/// comparison is signed integer ordering, and the canonical encoding is the
/// raw count as eight little-endian two's-complement bytes. Arithmetic never
/// wraps: addition, subtraction, and multiplication return `Overflow` when the
/// exact scaled result cannot fit. Multiplication truncates toward zero when
/// the mathematical result is between representable micro-units.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scalar(i64);

impl Scalar {
    pub const SCALE: i64 = 1_000_000;
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(Self::SCALE);

    pub const fn from_raw_microunits(raw: i64) -> Self {
        Self(raw)
    }

    pub const fn raw_microunits(self) -> i64 {
        self.0
    }

    pub const fn encode(self) -> [u8; SCALAR_ENCODED_LEN] {
        self.0.to_le_bytes()
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        let bytes: [u8; SCALAR_ENCODED_LEN] =
            encoded
                .try_into()
                .map_err(|_| InfoDecodeError::WrongLength {
                    expected: SCALAR_ENCODED_LEN,
                    actual: encoded.len(),
                })?;
        Ok(Self(i64::from_le_bytes(bytes)))
    }

    pub const fn checked_add(self, rhs: Self) -> Result<Self, ScalarArithmeticError> {
        match self.0.checked_add(rhs.0) {
            Some(value) => Ok(Self(value)),
            None => Err(ScalarArithmeticError::Overflow),
        }
    }

    pub const fn checked_sub(self, rhs: Self) -> Result<Self, ScalarArithmeticError> {
        match self.0.checked_sub(rhs.0) {
            Some(value) => Ok(Self(value)),
            None => Err(ScalarArithmeticError::Overflow),
        }
    }

    pub fn checked_mul(self, rhs: Self) -> Result<Self, ScalarArithmeticError> {
        let scaled = (i128::from(self.0) * i128::from(rhs.0)) / i128::from(Self::SCALE);
        i64::try_from(scaled)
            .map(Self)
            .map_err(|_| ScalarArithmeticError::Overflow)
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(SCALAR_INFO_ID, &self.encode())
    }
}

pub(crate) fn semantic_digest(info_id: &str, encoded: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(SEMANTIC_DIGEST_DOMAIN);
    hash.update((info_id.len() as u16).to_le_bytes());
    hash.update(info_id.as_bytes());
    hash.update(encoded);
    hash.finalize().into()
}
