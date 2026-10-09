//! Explicit extended-profile specialization of the shared quantity law.

use super::conversion::is_radian;
use super::conversion_law::{Arithmetic, Fraction};
use super::magnitude::{Magnitude, SignedMagnitude};
use super::{
    ExactDecimalQuantity, Quantity, QuantityConversionRefusal, QuantityDimension, QuantityUnit,
};
use core::cmp::Ordering;

struct WideArithmetic;

impl Arithmetic for WideArithmetic {
    type Number = SignedMagnitude;
    fn integer(value: i128) -> Self::Number {
        SignedMagnitude::from_i128(value)
    }
    fn add(left: Self::Number, right: Self::Number) -> Option<Self::Number> {
        left.checked_add(right)
    }
    fn multiply(left: Self::Number, right: Self::Number) -> Option<Self::Number> {
        left.checked_product(right)
    }
    fn negate(value: Self::Number) -> Option<Self::Number> {
        Some(value.negated())
    }
    fn compare(left: Self::Number, right: Self::Number) -> Ordering {
        left.cmp(right)
    }
    fn exact_i64(
        numerator: Self::Number,
        denominator: Self::Number,
    ) -> Result<i64, QuantityConversionRefusal> {
        let denominator = denominator.magnitude();
        // Match the legacy law's precision-before-range refusal order.
        if numerator
            .magnitude()
            .remainder(denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?
            != Magnitude::ZERO
        {
            return Err(QuantityConversionRefusal::Inexact);
        }
        let limit = if numerator.negative() {
            1_u64 << 63
        } else {
            i64::MAX as u64
        };
        let largest = denominator
            .checked_mul(Magnitude::from_u128(u128::from(limit)))
            .ok_or(QuantityConversionRefusal::Overflow)?;
        if numerator.magnitude().cmp(largest) == Ordering::Greater {
            return Err(QuantityConversionRefusal::Overflow);
        }
        let magnitude = numerator
            .magnitude()
            .exact_quotient(denominator, limit)
            .ok_or(QuantityConversionRefusal::Inexact)?;
        Ok(if numerator.negative() {
            if magnitude == 1_u64 << 63 {
                i64::MIN
            } else {
                -(magnitude as i64)
            }
        } else {
            magnitude as i64
        })
    }
}

fn decimal(
    value: ExactDecimalQuantity,
) -> Result<Fraction<WideArithmetic>, QuantityConversionRefusal> {
    let power = Magnitude::power_of_ten(value.exponent().unsigned_abs())
        .ok_or(QuantityConversionRefusal::Overflow)?;
    let coefficient = SignedMagnitude::from_i128(value.coefficient());
    let one = SignedMagnitude::from_i128(1);
    Ok(if value.exponent() >= 0 {
        Fraction::new(
            coefficient
                .checked_mul(power)
                .ok_or(QuantityConversionRefusal::Overflow)?,
            one,
        )
    } else {
        Fraction::new(
            coefficient,
            one.checked_mul(power)
                .ok_or(QuantityConversionRefusal::Overflow)?,
        )
    })
}

fn compatible(source: QuantityUnit, target: QuantityUnit) -> Result<(), QuantityConversionRefusal> {
    if source.dimension() != target.dimension() {
        return Err(QuantityConversionRefusal::IncompatibleDimensions);
    }
    if source.dimension() == QuantityDimension::Angle && is_radian(source) != is_radian(target) {
        return Err(QuantityConversionRefusal::Inexact);
    }
    Ok(())
}

pub(super) fn to_legacy(
    source: ExactDecimalQuantity,
    target: QuantityUnit,
) -> Result<Quantity, QuantityConversionRefusal> {
    compatible(source.unit(), target)?;
    decimal(source)?
        .into_canonical(source.unit())?
        .in_target(target)?
        .legacy_integer(target)
}

pub(super) fn compare(
    left: ExactDecimalQuantity,
    right: ExactDecimalQuantity,
) -> Result<Ordering, QuantityConversionRefusal> {
    compatible(left.unit(), right.unit())?;
    decimal(left)?
        .into_canonical(left.unit())?
        .compare(decimal(right)?.into_canonical(right.unit())?)
}
