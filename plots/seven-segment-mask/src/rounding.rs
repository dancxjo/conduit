//! The same exact conversion and bounded display rounding as graphical Masks.
use super::MaskUnavailable as R;
use conduit_core::{Quantity, Unit};
use conduit_face::{rounded_quantity_coordinate, QuantityFormatRefusal};

pub(super) fn rounded(value: Quantity, unit: Unit, places: u32) -> Result<i128, R> {
    let places = u8::try_from(places).map_err(|_| R::NumericExtent)?;
    rounded_quantity_coordinate(value, unit, places).map_err(|why| match why {
        QuantityFormatRefusal::Overflow => R::Overflow,
        _ => R::NumericExtent,
    })
}
