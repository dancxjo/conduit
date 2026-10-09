//! Exact rational legacy quantity conversion and comparison support.

use super::{Quantity, QuantityConversionRefusal, QuantityUnit};

pub(super) fn canonical_fraction(
    value: Quantity,
) -> Result<(i128, i128), QuantityConversionRefusal> {
    let (scale, offset, denominator) = value.unit.canonical_transform();
    let numerator = i128::from(value.value)
        .checked_mul(scale)
        .and_then(|value| value.checked_add(offset))
        .ok_or(QuantityConversionRefusal::Overflow)?;
    Ok((numerator, denominator))
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
    let (source_scale, source_offset, source_transform_denominator) = source.canonical_transform();
    let (target_scale, target_offset, target_denominator) = target.canonical_transform();
    let canonical_numerator = source_numerator
        .checked_mul(source_scale)
        .and_then(|value| {
            source_offset
                .checked_mul(source_denominator)?
                .checked_add(value)
        })
        .ok_or(QuantityConversionRefusal::Overflow)?;
    let canonical_denominator = source_denominator
        .checked_mul(source_transform_denominator)
        .ok_or(QuantityConversionRefusal::Overflow)?;
    let target_numerator = canonical_numerator
        .checked_mul(target_denominator)
        .and_then(|value| {
            target_offset
                .checked_mul(canonical_denominator)?
                .checked_neg()?
                .checked_add(value)
        })
        .ok_or(QuantityConversionRefusal::Overflow)?;
    let target_value_denominator = canonical_denominator
        .checked_mul(target_scale)
        .ok_or(QuantityConversionRefusal::Overflow)?;
    if target_numerator % target_value_denominator != 0 {
        return Err(QuantityConversionRefusal::Inexact);
    }
    let value = i64::try_from(target_numerator / target_value_denominator)
        .map_err(|_| QuantityConversionRefusal::Overflow)?;
    Ok(Quantity::new(value, target))
}
