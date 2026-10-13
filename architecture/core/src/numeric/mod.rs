//! Bounded exact mathematical denotations. Numerical approximation is a separate projection.
//! Owned arithmetic allocates during preparation; borrowed canonical admission does not.
use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
mod codec;
mod integer;
mod profile;
mod render;
mod serde_wire;
pub use profile::{NumericForms, NumericWireShape, PortableNumericProfile};
mod rational;
mod symbolic;
pub use codec::MathValueRef;
pub use rational::ExactRational;
pub use symbolic::{SymbolicDag, SymbolicNodeRef, SymbolicOperation};
pub const MAX_INTEGER_LIMBS: usize = 128;
pub const MAX_WORK_LIMBS: usize = 256;
pub const MAX_NUMERIC_WORK_STEPS: usize = 8192;
pub const MAX_SYMBOLIC_NODES: usize = 128;
pub const MAX_SYMBOLIC_DEPTH: usize = 32;
pub const MAX_NUMERIC_ENCODED_BYTES: usize = 32_768;
pub const MATH_VALUE_INFO_ID: &str = "value/scalar";
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum NumericRefusal {
    InvalidNumber,
    InvalidProfile,
    ProfileViolation,
    DivisionByZero,
    Domain,
    IndeterminateDomain,
    IntegerBudget,
    NodeBudget,
    DepthBudget,
    StorageBudget,
    WorkBudget,
    NonCanonical,
    MalformedEncoding,
    UnsupportedVersion,
    OutputTooSmall,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericProjectionRefusal {
    Inexact,
    Overflow,
    UnresolvedSymbolic,
    Budget(NumericRefusal),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalEquality {
    Equal,
    NotEqual,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathValue {
    SmallInteger(i64),
    Rational(Box<ExactRational>),
    Symbolic(Box<SymbolicDag>),
}
impl MathValue {
    pub const ZERO: Self = Self::SmallInteger(0);
    pub const ONE: Self = Self::SmallInteger(1);
    pub const fn integer(value: i64) -> Self {
        Self::SmallInteger(value)
    }
    pub fn from_integer_decimal(source: &str) -> Result<Self, NumericRefusal> {
        Ok(Self::from_rational(ExactRational::integer(source)?))
    }
    pub fn from_decimal_parts(coefficient: i128, exponent: i16) -> Result<Self, NumericRefusal> {
        if coefficient == 0 {
            return Ok(Self::ZERO);
        }
        let numerator = integer::Magnitude::from_u128(coefficient.unsigned_abs());
        let power = integer::Magnitude::power10(usize::from(exponent.unsigned_abs()))?;
        let value = if exponent >= 0 {
            ExactRational::new(
                coefficient < 0,
                numerator.mul(&power)?,
                integer::Magnitude::from_u64(1),
            )?
        } else {
            ExactRational::new(coefficient < 0, numerator, power)?
        };
        Ok(Self::from_rational(value))
    }
    pub fn parse_decimal(source: &str) -> Result<Self, NumericRefusal> {
        Ok(Self::from_rational(ExactRational::decimal(source)?))
    }
    pub fn rational(numerator: &str, denominator: &str) -> Result<Self, NumericRefusal> {
        Ok(Self::from_rational(ExactRational::ratio(
            numerator,
            denominator,
        )?))
    }
    pub fn from_rational(value: ExactRational) -> Self {
        match value.as_i64() {
            Some(n) => Self::SmallInteger(n),
            None => Self::Rational(Box::new(value)),
        }
    }
    pub fn as_rational(&self) -> Option<ExactRational> {
        match self {
            Self::SmallInteger(n) => Some(ExactRational::from_i64(*n)),
            Self::Rational(n) => Some((**n).clone()),
            Self::Symbolic(_) => None,
        }
    }
    pub fn exact_i64(&self) -> Option<i64> {
        match self {
            Self::SmallInteger(n) => Some(*n),
            Self::Rational(n) => n.as_i64(),
            Self::Symbolic(_) => None,
        }
    }
    pub fn checked_add(&self, other: &Self) -> Result<Self, NumericRefusal> {
        if let (Self::SmallInteger(a), Self::SmallInteger(b)) = (self, other) {
            if let Some(n) = a.checked_add(*b) {
                return Ok(Self::integer(n));
            }
        }
        if let (Some(a), Some(b)) = (self.as_rational(), other.as_rational()) {
            return Ok(Self::from_rational(a.checked_add(&b)?));
        }
        symbolic::binary(SymbolicOperation::Add, self, other)
    }
    pub fn checked_neg(&self) -> Result<Self, NumericRefusal> {
        if let Some(n) = self.as_rational() {
            return Ok(Self::from_rational(n.negated()));
        }
        symbolic::unary(SymbolicOperation::Negate, self)
    }
    pub fn checked_sub(&self, other: &Self) -> Result<Self, NumericRefusal> {
        if self == other {
            return Ok(Self::ZERO);
        }
        self.checked_add(&other.checked_neg()?)
    }
    pub fn checked_mul(&self, other: &Self) -> Result<Self, NumericRefusal> {
        if let (Self::SmallInteger(a), Self::SmallInteger(b)) = (self, other) {
            if let Some(n) = a.checked_mul(*b) {
                return Ok(Self::integer(n));
            }
        }
        if let (Some(a), Some(b)) = (self.as_rational(), other.as_rational()) {
            return Ok(Self::from_rational(a.checked_mul(&b)?));
        }
        symbolic::binary(SymbolicOperation::Multiply, self, other)
    }
    pub fn checked_div(&self, other: &Self) -> Result<Self, NumericRefusal> {
        if let (Some(a), Some(b)) = (self.as_rational(), other.as_rational()) {
            return Ok(Self::from_rational(a.checked_div(&b)?));
        }
        if !other.proven_nonzero() {
            return Err(if other == &Self::ZERO {
                NumericRefusal::DivisionByZero
            } else {
                NumericRefusal::IndeterminateDomain
            });
        }
        symbolic::binary(SymbolicOperation::Divide, self, other)
    }
    pub fn checked_floor_i64(&self) -> Result<i64, crate::NumericProjectionRefusal> {
        let bytes = self
            .canonical_bytes()
            .map_err(crate::NumericProjectionRefusal::Budget)?;
        MathValueRef::validate(&bytes)
            .map_err(crate::NumericProjectionRefusal::Budget)?
            .checked_floor_i64()
    }
    pub fn checked_pow(&self, exponent: i64) -> Result<Self, NumericRefusal> {
        if exponent.unsigned_abs() > 4096 {
            return Err(NumericRefusal::WorkBudget);
        }
        if exponent == 0 {
            if !self.proven_nonzero() {
                return Err(NumericRefusal::IndeterminateDomain);
            }
            return Ok(Self::ONE);
        }
        let mut remaining = exponent.unsigned_abs();
        let mut factor = self.clone();
        let mut result = Self::ONE;
        while remaining != 0 {
            if remaining & 1 != 0 {
                result = result.checked_mul(&factor)?;
            }
            remaining >>= 1;
            if remaining != 0 {
                factor = factor.checked_mul(&factor)?;
            }
        }
        if exponent < 0 {
            Self::ONE.checked_div(&result)
        } else {
            Ok(result)
        }
    }
    pub fn compare(&self, other: &Self) -> Result<Option<Ordering>, NumericRefusal> {
        if self == other {
            return Ok(Some(Ordering::Equal));
        }
        if let (Some(a), Some(b)) = (self.as_rational(), other.as_rational()) {
            return a.compare(&b).map(Some);
        }
        Ok(None)
    }
    pub fn mathematical_equality(
        &self,
        other: &Self,
    ) -> Result<MathematicalEquality, NumericRefusal> {
        Ok(match self.compare(other)? {
            Some(Ordering::Equal) => MathematicalEquality::Equal,
            Some(_) => MathematicalEquality::NotEqual,
            None => MathematicalEquality::Unknown,
        })
    }
    pub fn pi() -> Self {
        symbolic::pi()
    }
    pub fn checked_root(&self, degree: u8) -> Result<Self, NumericRefusal> {
        symbolic::root(self, degree)
    }
    /// Exact sine of a coordinate expressed in turns; this does not coerce a Scalar to Angle.
    pub fn sin_turns(&self) -> Result<Self, NumericRefusal> {
        symbolic::trig_turns(SymbolicOperation::Sin, self)
    }
    pub fn cos_turns(&self) -> Result<Self, NumericRefusal> {
        symbolic::trig_turns(SymbolicOperation::Cos, self)
    }
    pub fn tan_turns(&self) -> Result<Self, NumericRefusal> {
        symbolic::trig_turns(SymbolicOperation::Tan, self)
    }
    pub fn asin_turns(&self) -> Result<Self, NumericRefusal> {
        symbolic::inverse(SymbolicOperation::Asin, self)
    }
    pub fn acos_turns(&self) -> Result<Self, NumericRefusal> {
        symbolic::inverse(SymbolicOperation::Acos, self)
    }
    pub fn atan_turns(&self) -> Result<Self, NumericRefusal> {
        symbolic::inverse(SymbolicOperation::Atan, self)
    }
    pub fn atan2_turns(y: &Self, x: &Self) -> Result<Self, NumericRefusal> {
        symbolic::atan2(y, x)
    }
    pub fn proven_nonzero(&self) -> bool {
        match self.as_rational() {
            Some(n) => !n.is_zero(),
            None => symbolic::nonzero(self),
        }
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, NumericRefusal> {
        codec::encode(self)
    }
    pub fn encode_into(&self, output: &mut [u8]) -> Result<usize, NumericRefusal> {
        let bytes = self.canonical_bytes()?;
        if bytes.len() > output.len() {
            return Err(NumericRefusal::OutputTooSmall);
        }
        output[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
    pub fn decode(encoded: &[u8]) -> Result<Self, NumericRefusal> {
        codec::decode(encoded)
    }
    pub fn representation_identity(&self) -> Result<[u8; 32], NumericRefusal> {
        Ok(crate::semantic_digest(
            MATH_VALUE_INFO_ID,
            &self.canonical_bytes()?,
        ))
    }
}
#[cfg(test)]
mod tests;
