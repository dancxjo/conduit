//! Pure bounded quantity formatting; no sensor, Face owner, clock or effect grant.
use crate::{pattern, SegmentPattern, SevenSegmentShow, NUMERALS};
use conduit_core::{Quantity, Unit};
use conduit_face::{rounded_quantity_coordinate, QuantityFormatRefusal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityShowRefusal {
    Layout,
    Extent,
    Quantity(QuantityFormatRefusal),
}

/// Round in the selected unit once, then fit four cells with explicit labels.
/// Decimal points use their separate Show bits and consume no digit cell.
/// This prepares patterns only; it does not admit or manifest a Mask Show.
pub fn prepare_quantity_show(
    value: Quantity,
    unit: Unit,
    decimal_places: u8,
    prefix: &[SegmentPattern],
    suffix: &[SegmentPattern],
) -> Result<SevenSegmentShow, QuantityShowRefusal> {
    use QuantityShowRefusal as R;
    let labels = prefix.len().checked_add(suffix.len()).ok_or(R::Layout)?;
    if labels >= 4 || decimal_places > 3 {
        return Err(R::Layout);
    }
    let coordinate =
        rounded_quantity_coordinate(value, unit, decimal_places).map_err(R::Quantity)?;
    let mut magnitude = coordinate.unsigned_abs();
    let mut count = 1usize;
    let mut remaining = magnitude;
    while remaining >= 10 {
        remaining /= 10;
        count += 1;
    }
    count = count.max(usize::from(decimal_places) + 1);
    let negative = coordinate < 0;
    if count + usize::from(negative) > 4 - labels {
        return Err(R::Extent);
    }
    let mut digits = [pattern(0); 4];
    digits[..prefix.len()].copy_from_slice(prefix);
    let end = 4 - suffix.len();
    digits[end..].copy_from_slice(suffix);
    for place in 0..count {
        digits[end - 1 - place] = pattern(NUMERALS[(magnitude % 10) as usize]);
        magnitude /= 10;
    }
    if negative {
        digits[end - count - 1] = pattern(0x40);
    }
    let mut points = [false; 4];
    if decimal_places != 0 {
        points[end - usize::from(decimal_places) - 1] = true;
    }
    Ok(SevenSegmentShow::from_parts(digits, points, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SegmentPatternForm;
    fn bits(show: SevenSegmentShow) -> [u8; 4] {
        (*show.digits()).map(|g| SegmentPatternForm::encode(g)[0])
    }
    #[test]
    fn arbitrary_quantities_round_and_fit_without_sensor_policy() {
        let volts = Quantity::from_decimal(370, -2, Unit::Volt).unwrap();
        let show = prepare_quantity_show(volts, Unit::Volt, 2, &[], &[]).unwrap();
        assert_eq!(*show.decimal_points(), [false, true, false, false]);
        assert_eq!(bits(show), [0, NUMERALS[3], NUMERALS[7], NUMERALS[0]]);
        let pressure = Quantity::from_decimal(10134081, -2, Unit::Pascal).unwrap();
        let show =
            prepare_quantity_show(pressure, Unit::resolve("hPa").unwrap(), 0, &[], &[]).unwrap();
        assert_eq!(
            bits(show),
            [NUMERALS[1], NUMERALS[0], NUMERALS[1], NUMERALS[3]]
        );
        let temp = Quantity::from_decimal(2422, -2, Unit::Celsius).unwrap();
        let show = prepare_quantity_show(
            temp,
            Unit::Fahrenheit,
            0,
            &[],
            &[pattern(0x63), pattern(0x71)],
        )
        .unwrap();
        assert_eq!(bits(show), [NUMERALS[7], NUMERALS[6], 0x63, 0x71]);
        assert!(matches!(
            prepare_quantity_show(
                Quantity::new(100, Unit::Fahrenheit),
                Unit::Fahrenheit,
                0,
                &[],
                &[pattern(0x63), pattern(0x71)]
            ),
            Err(QuantityShowRefusal::Extent)
        ));
        assert!(prepare_quantity_show(volts, Unit::Celsius, 0, &[], &[]).is_err());
    }
}
