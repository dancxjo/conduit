//! Certified relative Time coordinates prepared outside the execution kernel.
use crate::{
    ExactNumericWork, PreparedQuantityIntegerEnclosure, PrimitiveInfoKind, QuantityDimension,
    QuantityIntegerEnclosureRefusal, QuantityRole, RationalQuantity, RationalQuantityRefusal, Unit,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolDurationPreparationRefusal {
    InvalidDuration,
    InvalidTolerance,
    Quantity(RationalQuantityRefusal),
    Projection(QuantityIntegerEnclosureRefusal),
}

/// A conservative fixed-width completion coordinate, with the exact original
/// Time, typed tolerance, conversion and outward-error witness still inspectable.
/// This certifies numeric containment, never a future completion deadline.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PreparedPoolDuration {
    source: RationalQuantity,
    tolerance: RationalQuantity,
    projection: PreparedQuantityIntegerEnclosure,
    upper_nanoseconds: u64,
}
impl PreparedPoolDuration {
    pub fn prepare(
        source: RationalQuantity,
        tolerance: RationalQuantity,
        work: &mut ExactNumericWork,
    ) -> Result<Self, PoolDurationPreparationRefusal> {
        use PoolDurationPreparationRefusal as R;
        if source.dimension() != QuantityDimension::Time
            || source.role() != QuantityRole::Linear
            || source.coordinate().is_negative()
        {
            return Err(R::InvalidDuration);
        }
        if tolerance.unit() != source.unit()
            || tolerance.role() != QuantityRole::Linear
            || tolerance.coordinate().is_negative()
        {
            return Err(R::InvalidTolerance);
        }
        let unit = Unit::resolve("ns").expect("reviewed common relative Time coordinate");
        let converted = source.convert_to_unit(unit, work).map_err(R::Quantity)?;
        let converted_tolerance = tolerance.convert_to_unit(unit, work).map_err(R::Quantity)?;
        let projection = PreparedQuantityIntegerEnclosure::prepare(
            converted,
            PrimitiveInfoKind::U64,
            converted_tolerance,
            work,
        )
        .map_err(R::Projection)?;
        let upper_nanoseconds = projection
            .target()
            .coordinate()
            .upper()
            .unsigned()
            .expect("sealed U64 projection")
            .try_into()
            .expect("sealed U64 width");
        Ok(Self {
            source,
            tolerance,
            projection,
            upper_nanoseconds,
        })
    }
    pub const fn source(self) -> RationalQuantity {
        self.source
    }
    pub const fn tolerance(self) -> RationalQuantity {
        self.tolerance
    }
    pub const fn projection(self) -> PreparedQuantityIntegerEnclosure {
        self.projection
    }
    pub const fn upper_nanoseconds(self) -> u64 {
        self.upper_nanoseconds
    }
}
