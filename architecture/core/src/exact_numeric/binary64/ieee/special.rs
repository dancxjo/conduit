//! Fixed-work exceptional cases precede exact finite arithmetic.
use super::{
    is_nan, is_signaling_nan, result, IeeeBinary64ExceptionFlags as Flags,
    IeeeBinary64Operation as Op, IeeeBinary64Result, INFINITY, QUIET, SIGN,
};

fn invalid() -> IeeeBinary64Result {
    result(
        SIGN | INFINITY | QUIET,
        Flags {
            invalid: true,
            ..Default::default()
        },
    )
}
pub(super) fn evaluate(operation: Op, left: u64, mut right: u64) -> Option<IeeeBinary64Result> {
    if is_nan(left) || is_nan(right) {
        return Some(result(
            (if is_nan(left) { left } else { right }) | QUIET,
            Flags {
                invalid: is_signaling_nan(left) || is_signaling_nan(right),
                ..Default::default()
            },
        ));
    }
    if operation == Op::Subtract {
        right ^= SIGN;
    }
    let left_infinite = left & !SIGN == INFINITY;
    let right_infinite = right & !SIGN == INFINITY;
    let left_zero = left & !SIGN == 0;
    let right_zero = right & !SIGN == 0;
    let sign = (left ^ right) & SIGN;
    Some(match operation {
        Op::Add | Op::Subtract => {
            if left_infinite && right_infinite && (left ^ right) & SIGN != 0 {
                invalid()
            } else if left_infinite {
                result(left, Default::default())
            } else if right_infinite {
                result(right, Default::default())
            } else {
                return None;
            }
        }
        Op::Multiply => {
            if (left_infinite && right_zero) || (right_infinite && left_zero) {
                invalid()
            } else if left_infinite || right_infinite {
                result(sign | INFINITY, Default::default())
            } else if left_zero || right_zero {
                result(sign, Default::default())
            } else {
                return None;
            }
        }
        Op::Divide => {
            if (left_infinite && right_infinite) || (left_zero && right_zero) {
                invalid()
            } else if left_infinite {
                result(sign | INFINITY, Default::default())
            } else if right_infinite {
                result(sign, Default::default())
            } else if right_zero {
                result(
                    sign | INFINITY,
                    Flags {
                        divide_by_zero: true,
                        ..Default::default()
                    },
                )
            } else if left_zero {
                result(sign, Default::default())
            } else {
                return None;
            }
        }
    })
}
