//! Checked finite operations preserve rounding at each selected format width.
//! Encoded equality is handled separately and preserves every IEEE bit class.
macro_rules! finite_operations {
    ($module:ident, $float:ty, $bytes:literal) => {
        pub(crate) mod $module {
            use crate::BinaryOperator;
            use core::cmp::Ordering;
            fn decode(bytes: &[u8]) -> Option<$float> {
                let value = <$float>::from_le_bytes(bytes.try_into().ok()?);
                value.is_finite().then_some(value)
            }
            pub(crate) fn arithmetic(
                operator: BinaryOperator,
                left: &[u8],
                right: &[u8],
            ) -> Option<[u8; $bytes]> {
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
            pub(crate) fn ordering(left: &[u8], right: &[u8]) -> Option<Ordering> {
                decode(left)?.partial_cmp(&decode(right)?)
            }
        }
    };
}
finite_operations!(binary32, f32, 4);
finite_operations!(binary64, f64, 8);
