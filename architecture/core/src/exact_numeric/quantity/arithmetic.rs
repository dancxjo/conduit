//! Exact non-affine arithmetic, retaining the left operand's admitted Unit.
use super::*;

impl RationalQuantity {
    pub fn checked_add(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        self.combine(other, false, work)
    }
    pub fn checked_subtract(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        self.combine(other, true, work)
    }
    fn combine(
        self,
        other: Self,
        subtract: bool,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        if self.role == QuantityRole::Point || self.role != other.role {
            return Err(RationalQuantityRefusal::Physical(
                QuantityConversionRefusal::IncompatibleQuantityRoles,
            ));
        }
        let other = other.convert_to_unit(self.unit, work)?;
        let coordinate = if subtract {
            self.coordinate.checked_subtract(other.coordinate, work)?
        } else {
            self.coordinate.checked_add(other.coordinate, work)?
        };
        Self::new(coordinate, self.unit, self.role)
    }
}
