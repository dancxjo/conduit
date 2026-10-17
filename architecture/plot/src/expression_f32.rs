//! Checked finite binary32 operations shared by reference and prepared owners.
//! Equality remains the existing IEEE encoded-value identity relation.
use crate::BinaryOperator;
use core::cmp::Ordering;
pub(super) fn decode(bytes: &[u8]) -> Option<f32> {
    let value = f32::from_le_bytes(bytes.try_into().ok()?);
    value.is_finite().then_some(value)
}
pub(super) fn arithmetic(operator: BinaryOperator, left: &[u8], right: &[u8]) -> Option<[u8; 4]> {
    let (left, right) = (decode(left)?, decode(right)?);
    let value = match operator {
        BinaryOperator::Add => left + right,
        BinaryOperator::Subtract => left - right,
        BinaryOperator::Multiply => left * right,
        BinaryOperator::Divide if right != 0. => left / right,
        BinaryOperator::Remainder if right != 0. => left % right,
        _ => return None,
    };
    value.is_finite().then(|| value.to_le_bytes())
}
pub(super) fn ordering(left: &[u8], right: &[u8]) -> Option<Ordering> {
    decode(left)?.partial_cmp(&decode(right)?)
}
