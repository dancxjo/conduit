//! Explicit representation semantics; ordinary mathematical projection is separate.
//! Integer arithmetic fixes rounding and exceptions independently of Host FP state.
use super::{Binary64Rounding, ExactNumericRefusal, ExactNumericWork, ExactRational, IeeeF64};
mod comparison;
mod special;
pub use comparison::*;
/// This profile fixes NaN propagation, quiet comparisons and flag layout.
pub const IEEE_BINARY64_REFERENCE_PROFILE: &str = "numeric/ieee-binary64-reference@1";
const SIGN: u64 = 1 << 63;
const INFINITY: u64 = 0x7ff0_0000_0000_0000;
const QUIET: u64 = 1 << 51;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IeeeBinary64Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// Per-operation flags, returned as data rather than changing ambient state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IeeeBinary64ExceptionFlags {
    pub invalid: bool,
    pub divide_by_zero: bool,
    pub overflow: bool,
    pub underflow: bool,
    pub inexact: bool,
}
impl IeeeBinary64ExceptionFlags {
    /// Explicit accumulation for an operation sequence; there are no global flags.
    pub const fn union(self, other: Self) -> Self {
        Self {
            invalid: self.invalid || other.invalid,
            divide_by_zero: self.divide_by_zero || other.divide_by_zero,
            overflow: self.overflow || other.overflow,
            underflow: self.underflow || other.underflow,
            inexact: self.inexact || other.inexact,
        }
    }
    /// Profile layout: inexact, underflow, overflow, divide-by-zero, invalid.
    pub const fn bits(self) -> u8 {
        self.inexact as u8
            | ((self.underflow as u8) << 1)
            | ((self.overflow as u8) << 2)
            | ((self.divide_by_zero as u8) << 3)
            | ((self.invalid as u8) << 4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IeeeBinary64Result {
    pub value: IeeeF64,
    pub flags: IeeeBinary64ExceptionFlags,
}
fn result(bits: u64, flags: IeeeBinary64ExceptionFlags) -> IeeeBinary64Result {
    IeeeBinary64Result {
        value: IeeeF64::from_bits(bits),
        flags,
    }
}
fn is_nan(bits: u64) -> bool {
    bits & !SIGN > INFINITY
}
fn is_signaling_nan(bits: u64) -> bool {
    is_nan(bits) && bits & QUIET == 0
}

/// Constant-work, rounding-independent exceptional cases of the IEEE profile.
/// Initialized native implementations share this NaN/zero/infinity convention.
/// `None` means the selected implementation must perform the arithmetic itself.
pub fn ieee_binary64_special_case(
    operation: IeeeBinary64Operation,
    left: IeeeF64,
    right: IeeeF64,
) -> Option<IeeeBinary64Result> {
    special::evaluate(operation, left.bits(), right.bits())
}

/// One binary64 operation, including nonfinite propagation and signed zero.
/// First-NaN propagation retains sign/payload and quiets signaling NaNs.
/// Invalid operations without a NaN operand produce 0xfff8000000000000.
/// Resource exhaustion is a distinct refusal, never a fabricated IEEE result.
/// Finite operands fit the existing bounded rational workspace; no heap is used.
pub fn evaluate_ieee_binary64(
    operation: IeeeBinary64Operation,
    left: IeeeF64,
    right: IeeeF64,
    rounding: Binary64Rounding,
    work: &mut ExactNumericWork,
) -> Result<IeeeBinary64Result, ExactNumericRefusal> {
    work.charge(1)?;
    if let Some(value) = ieee_binary64_special_case(operation, left, right) {
        return Ok(value);
    }
    let right_bits = right.bits()
        ^ if operation == IeeeBinary64Operation::Subtract {
            SIGN
        } else {
            0
        };
    let a = ExactRational::from_binary64(left, work)?;
    let b = ExactRational::from_binary64(IeeeF64::from_bits(right_bits), work)?;
    let source = match operation {
        IeeeBinary64Operation::Add | IeeeBinary64Operation::Subtract => a.checked_add(b, work)?,
        IeeeBinary64Operation::Multiply => a.checked_multiply(b, work)?,
        IeeeBinary64Operation::Divide => a.checked_divide(b, work)?,
    };
    if source == ExactRational::ZERO {
        // Only addition/subtraction reach here with zero. Other zero results
        // are handled with their multiplicative sign before rational decoding.
        let both_negative_zero = left.bits() == SIGN && right_bits == SIGN;
        let same_positive_zero = left.bits() == 0 && right_bits == 0;
        let negative = both_negative_zero
            || (!same_positive_zero && rounding == Binary64Rounding::TowardNegative);
        return Ok(result(if negative { SIGN } else { 0 }, Default::default()));
    }
    let (target, inexact) = match source.round_binary64(rounding, work) {
        Ok(value) => value,
        Err(ExactNumericRefusal::OutOfRange) => {
            let negative = source.is_negative();
            let infinite = match rounding {
                Binary64Rounding::NearestTiesEven => true,
                Binary64Rounding::TowardZero => false,
                Binary64Rounding::TowardPositive => !negative,
                Binary64Rounding::TowardNegative => negative,
            };
            return Ok(result(
                (if infinite { INFINITY } else { INFINITY - 1 }) | if negative { SIGN } else { 0 },
                IeeeBinary64ExceptionFlags {
                    overflow: true,
                    inexact: true,
                    ..Default::default()
                },
            ));
        }
        Err(why) => return Err(why),
    };
    let underflow = inexact && super::tininess::after_rounding(source, target, rounding, work)?;
    Ok(IeeeBinary64Result {
        value: target,
        flags: IeeeBinary64ExceptionFlags {
            underflow,
            inexact,
            ..Default::default()
        },
    })
}
