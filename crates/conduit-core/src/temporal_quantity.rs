//! Exact interop between monotonic durations and typed time quantities.

use crate::{MonotonicDuration, Quantity, QuantityConversionRefusal, QuantityUnit, TemporalScale};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TemporalQuantityRefusal {
    NegativeQuantity,
    Overflow,
    QuantityConversion(QuantityConversionRefusal),
}

impl TemporalScale {
    pub const fn quantity_unit(self) -> QuantityUnit {
        match self {
            Self::Seconds => QuantityUnit::Second,
            Self::Milliseconds => QuantityUnit::Millisecond,
            Self::Microseconds => QuantityUnit::Microsecond,
            Self::Nanoseconds => QuantityUnit::Nanosecond,
        }
    }
}

impl MonotonicDuration {
    pub fn from_quantity(
        quantity: Quantity,
        scale: TemporalScale,
    ) -> Result<Self, TemporalQuantityRefusal> {
        let converted = quantity
            .convert(scale.quantity_unit())
            .map_err(TemporalQuantityRefusal::QuantityConversion)?;
        let ticks = u64::try_from(converted.value())
            .map_err(|_| TemporalQuantityRefusal::NegativeQuantity)?;
        Ok(Self::new(ticks, scale))
    }

    pub fn quantity(self) -> Result<Quantity, TemporalQuantityRefusal> {
        let value = i64::try_from(self.ticks()).map_err(|_| TemporalQuantityRefusal::Overflow)?;
        Ok(Quantity::new(value, self.scale().quantity_unit()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QuantityDimension;

    #[test]
    fn temporal_scales_map_to_time_quantity_units() {
        for (scale, unit) in [
            (TemporalScale::Seconds, QuantityUnit::Second),
            (TemporalScale::Milliseconds, QuantityUnit::Millisecond),
            (TemporalScale::Microseconds, QuantityUnit::Microsecond),
            (TemporalScale::Nanoseconds, QuantityUnit::Nanosecond),
        ] {
            assert_eq!(scale.quantity_unit(), unit);
            assert_eq!(unit.dimension(), QuantityDimension::Time);
        }
    }

    #[test]
    fn duration_construction_converts_exact_time_quantities() {
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(2, QuantityUnit::Second),
                TemporalScale::Milliseconds,
            ),
            Ok(MonotonicDuration::new(2_000, TemporalScale::Milliseconds))
        );
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(2_000, QuantityUnit::Millisecond),
                TemporalScale::Seconds,
            ),
            Ok(MonotonicDuration::new(2, TemporalScale::Seconds))
        );
    }

    #[test]
    fn duration_construction_refuses_loss_and_wrong_dimensions() {
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(1, QuantityUnit::Nanosecond),
                TemporalScale::Microseconds,
            ),
            Err(TemporalQuantityRefusal::QuantityConversion(
                QuantityConversionRefusal::Inexact
            ))
        );
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(1, QuantityUnit::Hertz),
                TemporalScale::Seconds,
            ),
            Err(TemporalQuantityRefusal::QuantityConversion(
                QuantityConversionRefusal::IncompatibleDimensions
            ))
        );
    }

    #[test]
    fn duration_construction_refuses_negative_quantity() {
        assert_eq!(
            MonotonicDuration::from_quantity(
                Quantity::new(-1, QuantityUnit::Second),
                TemporalScale::Seconds,
            ),
            Err(TemporalQuantityRefusal::NegativeQuantity)
        );
    }

    #[test]
    fn duration_exports_exact_quantity_or_refuses_range_loss() {
        assert_eq!(
            MonotonicDuration::new(42, TemporalScale::Microseconds).quantity(),
            Ok(Quantity::new(42, QuantityUnit::Microsecond))
        );
        assert_eq!(
            MonotonicDuration::new(u64::MAX, TemporalScale::Nanoseconds).quantity(),
            Err(TemporalQuantityRefusal::Overflow)
        );
    }
}
