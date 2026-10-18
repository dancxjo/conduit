//! Quiet numerical predicates are separate from the carrier's bit identity.
use super::{
    is_nan, is_signaling_nan, ExactNumericRefusal, ExactNumericWork, IeeeBinary64ExceptionFlags,
    IeeeF64, SIGN,
};
use core::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IeeeBinary64QuietPredicate {
    Equal,
    Less,
    LessEqual,
    Unordered,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IeeeBinary64ComparisonResult {
    pub value: bool,
    pub flags: IeeeBinary64ExceptionFlags,
}
pub fn compare_ieee_binary64_quiet(
    predicate: IeeeBinary64QuietPredicate,
    left: IeeeF64,
    right: IeeeF64,
    work: &mut ExactNumericWork,
) -> Result<IeeeBinary64ComparisonResult, ExactNumericRefusal> {
    work.charge(1)?;
    let (left, right) = (left.bits(), right.bits());
    let unordered = is_nan(left) || is_nan(right);
    let order = if left == right || (left | right) & !SIGN == 0 {
        Ordering::Equal
    } else if (left ^ right) & SIGN != 0 {
        if left & SIGN != 0 {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    } else if left & SIGN != 0 {
        right.cmp(&left)
    } else {
        left.cmp(&right)
    };
    let value = match predicate {
        IeeeBinary64QuietPredicate::Equal => !unordered && order == Ordering::Equal,
        IeeeBinary64QuietPredicate::Less => !unordered && order == Ordering::Less,
        IeeeBinary64QuietPredicate::LessEqual => !unordered && order != Ordering::Greater,
        IeeeBinary64QuietPredicate::Unordered => unordered,
    };
    Ok(IeeeBinary64ComparisonResult {
        value,
        flags: IeeeBinary64ExceptionFlags {
            invalid: is_signaling_nan(left) || is_signaling_nan(right),
            ..Default::default()
        },
    })
}
