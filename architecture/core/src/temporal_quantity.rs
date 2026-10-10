//! Exact interop between monotonic durations and typed time quantities.

use crate::{MonotonicDuration, Quantity, QuantityConversionRefusal, TemporalScale, Unit};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TemporalQuantityRefusal {
    NegativeQuantity,
    Overflow,
    QuantityConversion(QuantityConversionRefusal),
}

impl TemporalScale {
    pub const fn quantity_unit(self) -> Unit {
        match self {
            Self::Seconds => Unit::Second,
            Self::Milliseconds => Unit::Millisecond,
            Self::Microseconds => Unit::Microsecond,
            Self::Nanoseconds => Unit::Nanosecond,
        }
    }
}

impl MonotonicDuration {
    pub fn from_quantity(
        quantity: Quantity,
        scale: TemporalScale,
    ) -> Result<Self, TemporalQuantityRefusal> {
        if quantity.coefficient() < 0 {
            return Err(TemporalQuantityRefusal::NegativeQuantity);
        }
        let ticks = quantity
            .convert_to_u64(scale.quantity_unit())
            .map_err(TemporalQuantityRefusal::QuantityConversion)?;
        Ok(Self::new(ticks, scale))
    }

    pub fn quantity(self) -> Result<Quantity, TemporalQuantityRefusal> {
        Quantity::from_decimal(i128::from(self.ticks()), 0, self.scale().quantity_unit())
            .map_err(|_| TemporalQuantityRefusal::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QuantityDimension;

    #[test]
    fn temporal_scales_map_to_time_quantity_units() {
        for (scale, unit) in [
            (TemporalScale::Seconds, Unit::Second),
            (TemporalScale::Milliseconds, Unit::Millisecond),
            (TemporalScale::Microseconds, Unit::Microsecond),
            (TemporalScale::Nanoseconds, Unit::Nanosecond),
        ] {
            assert_eq!(scale.quantity_unit(), unit);
            assert_eq!(unit.dimension(), QuantityDimension::Time);
        }
    }

    #[test]
    fn duration_construction_converts_exact_time_quantities() {
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(2, Unit::Second),
                TemporalScale::Milliseconds,
            ),
            Ok(MonotonicDuration::new(2_000, TemporalScale::Milliseconds))
        );
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(2_000, Unit::Millisecond),
                TemporalScale::Seconds,
            ),
            Ok(MonotonicDuration::new(2, TemporalScale::Seconds))
        );
    }

    #[test]
    fn duration_construction_refuses_loss_and_wrong_dimensions() {
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(1, Unit::Nanosecond),
                TemporalScale::Microseconds,
            ),
            Err(TemporalQuantityRefusal::QuantityConversion(
                QuantityConversionRefusal::Inexact
            ))
        );
        assert_eq!(
            MonotonicDuration::from_quantity(Quantity::new(1, Unit::Hertz), TemporalScale::Seconds,),
            Err(TemporalQuantityRefusal::QuantityConversion(
                QuantityConversionRefusal::IncompatibleDimensions
            ))
        );
    }

    #[test]
    fn duration_construction_refuses_negative_quantity() {
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(-1, Unit::Second),
                TemporalScale::Seconds,
            ),
            Err(TemporalQuantityRefusal::NegativeQuantity)
        );
    }

    #[test]
    fn duration_exports_the_full_unsigned_coordinate_exactly() {
        assert_eq!(
            MonotonicDuration::new(42, TemporalScale::Microseconds).quantity(),
            Ok(Quantity::new(42, Unit::Microsecond))
        );
        assert_eq!(
            MonotonicDuration::new(u64::MAX, TemporalScale::Nanoseconds).quantity(),
            Quantity::from_decimal(i128::from(u64::MAX), 0, Unit::Nanosecond)
                .map_err(|_| TemporalQuantityRefusal::Overflow)
        );
    }
}
