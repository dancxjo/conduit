//! Look-unit conversion reuses core laws; display rounding is explicit policy.
use super::MaskUnavailable as R;
use conduit_core::{
    ExactNumericWork, ExactRational, Quantity, QuantityConversionRefusal, RationalQuantity, Unit,
};

pub(super) fn rounded(value: Quantity, unit: Unit, places: u32) -> Result<i128, R> {
    match value.convert_to_decimal(unit) {
        Ok(value) => decimal(value, places),
        Err(QuantityConversionRefusal::Inexact) => rational(value, unit, places),
        Err(QuantityConversionRefusal::Overflow) => Err(R::Overflow),
        Err(_) => Err(R::NumericExtent),
    }
}
fn signed(magnitude: u128, negative: bool) -> Result<i128, R> {
    if negative && magnitude == 1u128 << 127 {
        return Ok(i128::MIN);
    }
    let magnitude = i128::try_from(magnitude).map_err(|_| R::Overflow)?;
    Ok(if negative { -magnitude } else { magnitude })
}
fn fraction(numerator: u128, denominator: u128, negative: bool) -> Result<i128, R> {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let magnitude = quotient
        .checked_add(u128::from(remainder >= denominator - remainder))
        .ok_or(R::Overflow)?;
    signed(magnitude, negative)
}
fn decimal(value: Quantity, places: u32) -> Result<i128, R> {
    let coefficient = value.coefficient();
    if coefficient == 0 {
        return Ok(0);
    }
    let exponent = i32::from(value.exponent()) + places as i32;
    if exponent >= 0 {
        let scale = 10_i128.checked_pow(exponent as u32).ok_or(R::Overflow)?;
        coefficient.checked_mul(scale).ok_or(R::Overflow)
    } else if exponent < -38 {
        // The largest i128 magnitude is below half of 10^39.
        Ok(0)
    } else {
        fraction(
            coefficient.unsigned_abs(),
            10_u128.pow((-exponent) as u32),
            coefficient < 0,
        )
    }
}
// A nonterminating unit conversion (e.g. Fahrenheit to Celsius) needs the
// existing finite exact rational profile. Keep its wider scratch out of the
// ordinary decimal-only formatter frame, including the DHT11 °C → °F path.
#[inline(never)]
fn rational(value: Quantity, unit: Unit, places: u32) -> Result<i128, R> {
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let coordinate = RationalQuantity::from_quantity(value, &mut work)
        .and_then(|value| value.convert_to_unit(unit, &mut work))
        .map_err(|_| R::NumericExtent)?
        .coordinate();
    let coordinate = coordinate
        .checked_multiply(ExactRational::from_u128(10_u128.pow(places)), &mut work)
        .map_err(|_| R::NumericExtent)?;
    let magnitude = if coordinate.is_negative() {
        coordinate.negated()
    } else {
        coordinate
    };
    let (numerator, denominator) = magnitude
        .checked_unsigned_ratio()
        .map_err(|_| R::NumericExtent)?;
    fraction(numerator, denominator, coordinate.is_negative())
}
