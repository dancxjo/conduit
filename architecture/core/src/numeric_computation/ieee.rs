//! Explicit decimal-to-IEEE literal custody uses the reviewed software projection.
use super::*;
use crate::numeric_realization::{
    Binary64Flags, Binary64Rounding, Binary64Value, NumericRealizationWork,
};

pub fn ieee_rounded_literal(
    source: &MathValue,
) -> Result<NumericComputationOperation, NumericComputationRefusal> {
    let mut work = NumericRealizationWork::new(1_000_000);
    let result = Binary64Value::project(source, Binary64Rounding::NearestEven, &mut work)
        .map_err(NumericComputationRefusal::InvalidLiteral)?;
    Ok(NumericComputationOperation::IeeeRoundedLiteral {
        source: source
            .canonical_bytes()
            .map_err(NumericComputationRefusal::InvalidLiteral)?,
        bits: result.value.bits(),
        flags: result.flags,
    })
}
pub(super) fn validate(
    source: &[u8],
    bits: u64,
    flags: Binary64Flags,
) -> Result<(), NumericComputationRefusal> {
    let value = MathValue::decode(source).map_err(NumericComputationRefusal::InvalidLiteral)?;
    if ieee_rounded_literal(&value)?
        != (NumericComputationOperation::IeeeRoundedLiteral {
            source: source.into(),
            bits,
            flags,
        })
    {
        return Err(NumericComputationRefusal::InvalidSemanticKind);
    }
    Ok(())
}
pub(super) fn flags_byte(flags: Binary64Flags) -> u8 {
    u8::from(flags.invalid)
        | (u8::from(flags.division_by_zero) << 1)
        | (u8::from(flags.overflow) << 2)
        | (u8::from(flags.underflow) << 3)
        | (u8::from(flags.inexact) << 4)
}
pub(super) fn decode_flags(byte: u8) -> Result<Binary64Flags, NumericComputationRefusal> {
    if byte & !31 != 0 {
        return Err(NumericComputationRefusal::MalformedEncoding);
    }
    Ok(Binary64Flags {
        invalid: byte & 1 != 0,
        division_by_zero: byte & 2 != 0,
        overflow: byte & 4 != 0,
        underflow: byte & 8 != 0,
        inexact: byte & 16 != 0,
    })
}
