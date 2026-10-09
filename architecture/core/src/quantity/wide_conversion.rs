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

/// Reduce the exact target coordinate before admitting the finite decimal
/// profile. A denominator with any remaining factor other than 2 or 5 cannot
/// produce a finite decimal; that is an inexact refusal, never rounding.
pub(super) fn to_decimal(
    source: ExactDecimalQuantity,
    target: QuantityUnit,
) -> Result<ExactDecimalQuantity, QuantityConversionRefusal> {
    compatible(source.unit(), target)?;
    project_decimal(
        decimal(source)?
            .into_canonical(source.unit())?
            .in_target(target)?,
        target,
    )
}

pub(super) fn to_target_decimal(
    source: ExactDecimalQuantity,
    target: QuantityUnit,
    exponent: i16,
) -> Result<ExactDecimalQuantity, QuantityConversionRefusal> {
    target_decimal(source, target, exponent, false)
}

pub(super) fn difference_to_target_decimal(
    source: ExactDecimalQuantity,
    target: QuantityUnit,
    exponent: i16,
) -> Result<ExactDecimalQuantity, QuantityConversionRefusal> {
    target_decimal(source, target, exponent, true)
}

fn temperature_difference_transform(unit: QuantityUnit) -> (i128, i128, i128) {
    let (scale, _, denominator) = unit.canonical_transform();
    (scale, 0, denominator)
}

pub(super) fn compare_differences(
    left: ExactDecimalQuantity,
    right: ExactDecimalQuantity,
) -> Result<Ordering, QuantityConversionRefusal> {
    compatible(left.unit(), right.unit())?;
    decimal(left)?
        .with_source_transform(temperature_difference_transform(left.unit()))?
        .compare(
            decimal(right)?
                .with_source_transform(temperature_difference_transform(right.unit()))?,
        )
}

fn target_decimal(
    source: ExactDecimalQuantity,
    target: QuantityUnit,
    exponent: i16,
    difference: bool,
) -> Result<ExactDecimalQuantity, QuantityConversionRefusal> {
    compatible(source.unit(), target)?;
    let transform = |unit: QuantityUnit| {
        if difference {
            temperature_difference_transform(unit)
        } else {
            unit.canonical_transform()
        }
    };
    let (mut numerator, mut denominator) = decimal(source)?
        .with_source_transform(transform(source.unit()))?
        .with_target_transform(transform(target))?
        .parts();
    let power = Magnitude::power_of_ten(exponent.unsigned_abs())
        .ok_or(QuantityConversionRefusal::Overflow)?;
    if exponent >= 0 {
        denominator = denominator
            .checked_mul(power)
            .ok_or(QuantityConversionRefusal::Overflow)?;
    } else {
        numerator = numerator
            .checked_mul(power)
            .ok_or(QuantityConversionRefusal::Overflow)?;
    }
    project_decimal(Fraction::new(numerator, denominator), target)
}

fn project_decimal(
    coordinate: Fraction<WideArithmetic>,
    target: QuantityUnit,
) -> Result<ExactDecimalQuantity, QuantityConversionRefusal> {
    let (numerator, denominator) = coordinate.parts();
    let mut left = numerator.magnitude();
    let mut right = denominator.magnitude();
    while right != Magnitude::ZERO {
        let remainder = left
            .remainder(right)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        left = right;
        right = remainder;
    }
    let mut coefficient = numerator
        .magnitude()
        .divide(left)
        .ok_or(QuantityConversionRefusal::Overflow)?
        .0;
    let mut divisor = denominator
        .magnitude()
        .divide(left)
        .ok_or(QuantityConversionRefusal::Overflow)?
        .0;
    let mut twos = 0_i16;
    let mut fives = 0_i16;
    for (factor, count) in [(2, &mut twos), (5, &mut fives)] {
        loop {
            let (quotient, remainder) = divisor
                .divide_small(factor)
                .ok_or(QuantityConversionRefusal::Overflow)?;
            if remainder != 0 {
                break;
            }
            divisor = quotient;
            *count += 1;
        }
    }
    if divisor != Magnitude::from_u128(1) {
        return Err(QuantityConversionRefusal::Inexact);
    }
    let places = twos.max(fives);
    for (factor, count) in [(2, places - twos), (5, places - fives)] {
        for _ in 0..count {
            coefficient = coefficient
                .checked_mul_small(factor)
                .ok_or(QuantityConversionRefusal::Overflow)?;
        }
    }
    let mut exponent = -places;
    if coefficient == Magnitude::ZERO {
        exponent = 0;
    }
    while coefficient != Magnitude::ZERO && exponent < super::EXACT_DECIMAL_MAX_EXPONENT {
        let (quotient, remainder) = coefficient
            .divide_small(10)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        if remainder != 0 {
            break;
        }
        coefficient = quotient;
        exponent += 1;
    }
    let magnitude = coefficient
        .to_u128()
        .and_then(|value| i128::try_from(value).ok())
        .ok_or(QuantityConversionRefusal::Overflow)?;
    let coefficient = if numerator.negative() {
        -magnitude
    } else {
        magnitude
    };
    ExactDecimalQuantity::new(coefficient, exponent, target)
        .map_err(|_| QuantityConversionRefusal::Overflow)
}
