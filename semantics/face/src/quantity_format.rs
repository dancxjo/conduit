//! Finite presentation rounding over Core's exact quantity and affine unit laws.
//! Neither a rounded label nor a selected unit replaces its source quantity.
use conduit_core::{
    ExactNumericRefusal, ExactNumericWork, ExactRational, IntegerEnclosureRefusal,
    PreparedIntegerEnclosure, PrimitiveInfoKind, Quantity, QuantityConversionRefusal,
    RationalQuantity, Unit,
};

pub const MAX_QUANTITY_DISPLAY_DECIMAL_PLACES: u8 = 9;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantityFormatRefusal {
    DecimalPlaces,
    UnsupportedQuantity,
    Overflow,
    NumericExtent,
}

/// Round once, nearest with ties away from zero, after exact conversion.
/// The returned integer has `decimal_places` implied decimal places. Rational
/// conversion uses the existing finite Core work profile rather than floats.
pub fn rounded_quantity_coordinate(
    value: Quantity,
    unit: Unit,
    decimal_places: u8,
) -> Result<i128, QuantityFormatRefusal> {
    if decimal_places > MAX_QUANTITY_DISPLAY_DECIMAL_PLACES {
        return Err(QuantityFormatRefusal::DecimalPlaces);
    }
    match value.convert_to_decimal(unit) {
        Ok(value) => decimal(value, decimal_places),
        // The exact decimal intermediate can exceed its coefficient profile
        // even when the final rounded label is small (tiny affine inputs).
        Err(QuantityConversionRefusal::Inexact | QuantityConversionRefusal::Overflow) => {
            rational(value, unit, decimal_places)
        }
        Err(_) => Err(QuantityFormatRefusal::UnsupportedQuantity),
    }
}

fn signed(magnitude: u128, negative: bool) -> Result<i128, QuantityFormatRefusal> {
    if negative && magnitude == 1u128 << 127 {
        return Ok(i128::MIN);
    }
    let magnitude = i128::try_from(magnitude).map_err(|_| QuantityFormatRefusal::Overflow)?;
    Ok(if negative { -magnitude } else { magnitude })
}
fn fraction(
    numerator: u128,
    denominator: u128,
    negative: bool,
) -> Result<i128, QuantityFormatRefusal> {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let magnitude = quotient
        .checked_add(u128::from(remainder >= denominator - remainder))
        .ok_or(QuantityFormatRefusal::Overflow)?;
    signed(magnitude, negative)
}
fn decimal(value: Quantity, places: u8) -> Result<i128, QuantityFormatRefusal> {
    let coefficient = value.coefficient();
    if coefficient == 0 {
        return Ok(0);
    }
    let exponent = i32::from(value.exponent()) + i32::from(places);
    if exponent >= 0 {
        let scale = 10_i128
            .checked_pow(exponent as u32)
            .ok_or(QuantityFormatRefusal::Overflow)?;
        coefficient
            .checked_mul(scale)
            .ok_or(QuantityFormatRefusal::Overflow)
    } else if exponent < -38 {
        Ok(0) // Every admitted coefficient magnitude is below half of 10^39.
    } else {
        fraction(
            coefficient.unsigned_abs(),
            10_u128.pow((-exponent) as u32),
            coefficient < 0,
        )
    }
}
// Keep the wider exact scratch off ordinary decimal-only formatter frames.
#[inline(never)]
fn rational(value: Quantity, unit: Unit, places: u8) -> Result<i128, QuantityFormatRefusal> {
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let coordinate = RationalQuantity::from_quantity(value, &mut work)
        .and_then(|value| value.convert_to_unit(unit, &mut work))
        .map_err(|_| QuantityFormatRefusal::NumericExtent)?
        .coordinate();
    let coordinate = coordinate
        .checked_multiply(
            ExactRational::from_u128(10_u128.pow(u32::from(places))),
            &mut work,
        )
        .map_err(|_| QuantityFormatRefusal::NumericExtent)?;
    let magnitude = if coordinate.is_negative() {
        coordinate.negated()
    } else {
        coordinate
    };
    // floor(abs(coordinate) + 1/2) is nearest, ties away from zero.
    // Divide in Core's wide exact profile before narrowing the integer; large
    // numerator/denominator components do not imply a large display result.
    let half =
        ExactRational::ratio(1, 2, &mut work).map_err(|_| QuantityFormatRefusal::NumericExtent)?;
    let shifted = magnitude
        .checked_add(half, &mut work)
        .map_err(|_| QuantityFormatRefusal::NumericExtent)?;
    let enclosure = PreparedIntegerEnclosure::prepare(
        shifted,
        PrimitiveInfoKind::U128,
        ExactRational::ONE,
        &mut work,
    )
    .map_err(|why| match why {
        IntegerEnclosureRefusal::Numeric(ExactNumericRefusal::OutOfRange)
        | IntegerEnclosureRefusal::Target(_) => QuantityFormatRefusal::Overflow,
        _ => QuantityFormatRefusal::NumericExtent,
    })?;
    let rounded = enclosure
        .target()
        .lower()
        .unsigned()
        .map_err(|_| QuantityFormatRefusal::Overflow)?;
    signed(rounded, coordinate.is_negative())
}
