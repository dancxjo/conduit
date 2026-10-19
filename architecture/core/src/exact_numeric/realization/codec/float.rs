//! Wire operation order and admitted bounds; decoding reruns the sealed algorithm.
use super::*;
use crate::{Binary64ErrorClaim, IeeeF64};
pub(super) fn mode(mode: Binary64Rounding) -> u8 {
    match mode {
        Binary64Rounding::NearestTiesEven => 0,
        Binary64Rounding::TowardZero => 1,
        Binary64Rounding::TowardPositive => 2,
        Binary64Rounding::TowardNegative => 3,
    }
}
pub(super) fn read_mode(
    cursor: &mut Cursor<'_>,
) -> Result<Binary64Rounding, NumericalRealizationRefusal> {
    Ok(match cursor.take(1)? {
        [0] => Binary64Rounding::NearestTiesEven,
        [1] => Binary64Rounding::TowardZero,
        [2] => Binary64Rounding::TowardPositive,
        [3] => Binary64Rounding::TowardNegative,
        _ => return Err(NumericalRealizationRefusal::InvalidCertificate),
    })
}
pub(super) fn encode(
    bytes: &mut Vec<u8>,
    value: &PreparedBinary64Algorithm,
) -> Result<(), NumericalRealizationRefusal> {
    bytes.push(value.limits().maximum_steps);
    bytes.extend_from_slice(&value.limits().maximum_retained_bytes.to_le_bytes());
    bytes.push(value.steps().len() as u8);
    for step in value.steps() {
        use Binary64Operation as O;
        match step.operation() {
            O::Constant(q) => {
                bytes.push(0);
                rational(bytes, **q)?;
            }
            O::IeeeConstant(bits) => {
                bytes.push(1);
                bytes.extend_from_slice(&bits.encode());
            }
            O::ProjectedConstant(input) => {
                bytes.push(7);
                rational(bytes, input.source())?;
                bytes.extend_from_slice(&input.target().encode());
                rational(bytes, input.maximum_error())?;
            }
            O::Add { left, right }
            | O::Subtract { left, right }
            | O::Multiply { left, right }
            | O::Divide { left, right } => {
                bytes.push(match step.operation() {
                    O::Add { .. } => 2,
                    O::Subtract { .. } => 3,
                    O::Multiply { .. } => 4,
                    _ => 5,
                });
                bytes.extend_from_slice(&[*left, *right]);
            }
            O::Negate { input } => bytes.extend_from_slice(&[6, *input]),
        }
    }
    bytes.extend_from_slice(&value.target().encode());
    bytes.push(match value.error() {
        Binary64ErrorClaim::ExactDistance(_) => 0,
        Binary64ErrorClaim::CertifiedBound(_) => 1,
    });
    Ok(())
}
pub(super) fn program(
    cursor: &mut Cursor<'_>,
    work: &mut ExactNumericWork,
) -> Result<(Vec<Binary64Operation>, Binary64AlgorithmLimits), NumericalRealizationRefusal> {
    let maximum_steps = cursor.take(1)?[0];
    let maximum_retained_bytes = u32::from_le_bytes(
        cursor
            .take(4)?
            .try_into()
            .map_err(|_| NumericalRealizationRefusal::InvalidCertificate)?,
    );
    let count = cursor.take(1)?[0] as usize;
    if count == 0 || count > crate::BINARY64_ALGORITHM_MAX_STEPS {
        return Err(NumericalRealizationRefusal::InvalidCertificate);
    }
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        use Binary64Operation as O;
        result.push(match cursor.take(1)?[0] {
            0 => O::Constant(Box::new(cursor.rational(work)?)),
            1 => O::IeeeConstant(IeeeF64::from_bits(cursor.u64()?)),
            tag @ 2..=5 => {
                let operands = cursor.take(2)?;
                let (left, right) = (operands[0], operands[1]);
                match tag {
                    2 => O::Add { left, right },
                    3 => O::Subtract { left, right },
                    4 => O::Multiply { left, right },
                    _ => O::Divide { left, right },
                }
            }
            6 => O::Negate {
                input: cursor.take(1)?[0],
            },
            7 => {
                let source = cursor.rational(work)?;
                let target = IeeeF64::from_bits(cursor.u64()?);
                let maximum_error = cursor.rational(work)?;
                O::ProjectedConstant(Box::new(crate::Binary64ProjectedInput::new(
                    source,
                    target,
                    maximum_error,
                    work,
                )?))
            }
            _ => return Err(NumericalRealizationRefusal::InvalidCertificate),
        });
    }
    Ok((
        result,
        Binary64AlgorithmLimits {
            maximum_steps,
            maximum_retained_bytes,
        },
    ))
}
pub(super) fn matches(
    cursor: &mut Cursor<'_>,
    value: &PreparedBinary64Algorithm,
) -> Result<bool, NumericalRealizationRefusal> {
    let bits = cursor.u64()?;
    let claim = cursor.take(1)?[0];
    Ok(bits == value.target().bits()
        && claim
            == match value.error() {
                Binary64ErrorClaim::ExactDistance(_) => 0,
                Binary64ErrorClaim::CertifiedBound(_) => 1,
            })
}
