//! Legacy quantity conversion retains its original i128 arithmetic profile.

use super::conversion_law::{Arithmetic, Fraction};
use super::{Quantity, QuantityConversionRefusal, QuantityUnit};
use core::cmp::Ordering;

struct LegacyArithmetic;

impl Arithmetic for LegacyArithmetic {
    type Number = i128;
    fn integer(value: i128) -> Self::Number {
        value
    }
    fn add(left: i128, right: i128) -> Option<i128> {
        left.checked_add(right)
    }
    fn multiply(left: i128, right: i128) -> Option<i128> {
        left.checked_mul(right)
    }
    fn negate(value: i128) -> Option<i128> {
        value.checked_neg()
    }
    fn compare(left: i128, right: i128) -> Ordering {
        left.cmp(&right)
    }
    fn exact_i64(numerator: i128, denominator: i128) -> Result<i64, QuantityConversionRefusal> {
        if numerator % denominator != 0 {
            return Err(QuantityConversionRefusal::Inexact);
        }
        i64::try_from(numerator / denominator).map_err(|_| QuantityConversionRefusal::Overflow)
    }
}

pub(super) const fn is_radian(unit: QuantityUnit) -> bool {
    matches!(
        unit,
        QuantityUnit::Microradian | QuantityUnit::Milliradian | QuantityUnit::Radian
    )
}

pub(super) fn convert_exact_rational(
    source_numerator: i128,
    source_denominator: i128,
    source: QuantityUnit,
    target: QuantityUnit,
) -> Result<Quantity, QuantityConversionRefusal> {
    Fraction::<LegacyArithmetic>::rational(source_numerator, source_denominator)?
        .into_canonical(source)?
        .in_target(target)?
        .legacy_integer(target)
}

pub(super) fn compare_legacy(
    left: Quantity,
    right: Quantity,
) -> Result<Ordering, QuantityConversionRefusal> {
    Fraction::<LegacyArithmetic>::rational(i128::from(left.value()), 1)?
        .into_canonical(left.unit())?
        .compare(
            Fraction::<LegacyArithmetic>::rational(i128::from(right.value()), 1)?
                .into_canonical(right.unit())?,
        )
}
