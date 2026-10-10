//! Fixed-capacity exact Quantity conversion and checked integer projections.

use super::conversion_law::{Arithmetic, Fraction};
use super::magnitude::{Magnitude, SignedMagnitude};
use super::{Quantity, QuantityConversionRefusal};
use crate::{DefinitionScalar, QuantityRole, Unit};
use core::cmp::Ordering;

struct WideArithmetic;

impl Arithmetic for WideArithmetic {
    type Number = SignedMagnitude;
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
        // Refuse fractional precision before checking integer range.
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

fn scalar(
    value: DefinitionScalar,
    binary: u16,
) -> Result<Fraction<WideArithmetic>, QuantityConversionRefusal> {
    let mut n = SignedMagnitude::from_i128(value.numerator);
    let mut d = SignedMagnitude::from_i128(value.denominator);
    let power = Magnitude::power_of_ten(value.decimal_exponent.unsigned_abs())
        .ok_or(QuantityConversionRefusal::Overflow)?;
    if value.decimal_exponent >= 0 {
        n = n
            .checked_mul(power)
            .ok_or(QuantityConversionRefusal::Overflow)?;
    } else {
        d = d
            .checked_mul(power)
            .ok_or(QuantityConversionRefusal::Overflow)?;
    }
    for _ in 0..binary {
        n = n
            .checked_mul(Magnitude::from_u128(2))
            .ok_or(QuantityConversionRefusal::Overflow)?;
    }
    Ok(Fraction::new(n, d))
}
fn compatible(source: Quantity, target: Unit) -> Result<(), QuantityConversionRefusal> {
    if source.dimension() != target.dimension() {
        return Err(QuantityConversionRefusal::IncompatibleDimensions);
    }
    if source.family().identity() != target.family().identity() {
        return Err(QuantityConversionRefusal::IncompatibleQuantityFamilies);
    }
    if !target.family().admits(source.role()) {
        return Err(QuantityConversionRefusal::IncompatibleQuantityRoles);
    }
    if source.unit().reference_anchor() != target.reference_anchor() {
        return Err(QuantityConversionRefusal::Inexact);
    }
    Ok(())
}
fn reference(source: Quantity) -> Result<Fraction<WideArithmetic>, QuantityConversionRefusal> {
    scalar(
        DefinitionScalar {
            numerator: source.coefficient(),
            denominator: 1,
            decimal_exponent: source.exponent(),
        },
        0,
    )?
    .multiply(scalar(
        source.unit().exact_scale(),
        source.unit().binary_exponent(),
    )?)?
    .add(scalar(
        source
            .unit()
            .exact_offset(source.role())
            .map_err(|_| QuantityConversionRefusal::IncompatibleQuantityRoles)?,
        0,
    )?)
}
pub(super) fn to_i64(source: Quantity, target: Unit) -> Result<i64, QuantityConversionRefusal> {
    target_fraction(source, target, false)?.integer()
}
pub(super) fn compare(
    left: Quantity,
    right: Quantity,
) -> Result<Ordering, QuantityConversionRefusal> {
    compatible(left, right.unit())?;
    if left.role() != right.role() {
        return Err(QuantityConversionRefusal::IncompatibleQuantityRoles);
    }
    reference(left)?.compare(reference(right)?)
}
/// Reduce the exact target coordinate before admitting the finite decimal
/// profile. A denominator with any remaining factor other than 2 or 5 cannot
/// produce a finite decimal; that is an inexact refusal, never rounding.
pub(super) fn to_decimal(
    source: Quantity,
    target: Unit,
) -> Result<Quantity, QuantityConversionRefusal> {
    target_decimal(source, target, 0, false)
}

pub(super) fn to_target_decimal(
    source: Quantity,
    target: Unit,
    exponent: i16,
) -> Result<Quantity, QuantityConversionRefusal> {
    target_decimal(source, target, exponent, false)
}

pub(super) fn difference_to_target_decimal(
    source: Quantity,
    target: Unit,
    exponent: i16,
) -> Result<Quantity, QuantityConversionRefusal> {
    target_decimal(source, target, exponent, true)
}

pub(super) fn compare_differences(
    left: Quantity,
    right: Quantity,
) -> Result<Ordering, QuantityConversionRefusal> {
    compare(left, right)
}
fn target_decimal(
    source: Quantity,
    target: Unit,
    exponent: i16,
    difference: bool,
) -> Result<Quantity, QuantityConversionRefusal> {
    let (mut numerator, mut denominator) = target_fraction(source, target, difference)?.parts();
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
    project_decimal(Fraction::new(numerator, denominator), target, source.role())
}

fn project_decimal(
    coordinate: Fraction<WideArithmetic>,
    target: Unit,
    role: QuantityRole,
) -> Result<Quantity, QuantityConversionRefusal> {
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
    while coefficient != Magnitude::ZERO && exponent < super::QUANTITY_MAX_EXPONENT {
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
    Quantity::from_decimal_role(coefficient, exponent, target, role)
        .map_err(|_| QuantityConversionRefusal::Overflow)
}

fn target_fraction(
    source: Quantity,
    target: Unit,
    _difference: bool,
) -> Result<Fraction<WideArithmetic>, QuantityConversionRefusal> {
    compatible(source, target)?;
    reference(source)?
        .add(
            scalar(
                target
                    .exact_offset(source.role())
                    .map_err(|_| QuantityConversionRefusal::IncompatibleQuantityRoles)?,
                0,
            )?
            .negate()?,
        )?
        .divide(scalar(target.exact_scale(), target.binary_exponent())?)
}
