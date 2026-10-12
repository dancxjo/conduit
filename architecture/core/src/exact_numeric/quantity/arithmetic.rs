//! Exact role-aware arithmetic. Results retain the left Unit, except Delta +
//! Point, which retains the Point operand's Unit. Point offsets never scale Delta.
use super::*;

impl RationalQuantity {
    /// Add matching Linear/Delta values or translate a Point by a Delta.
    /// Two Points cannot be added. Conversion and arithmetic consume finite fuel.
    pub fn checked_add(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        self.combine(other, false, work)
    }
    /// Subtract matching Linear/Delta values, translate a Point by a Delta,
    /// or form a Delta from two compatible Points. Delta minus Point refuses.
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
        let role = match (self.role, other.role, subtract) {
            (QuantityRole::Linear, QuantityRole::Linear, _)
            | (QuantityRole::Delta, QuantityRole::Delta, _) => self.role,
            (QuantityRole::Point, QuantityRole::Point, true) => QuantityRole::Delta,
            (QuantityRole::Point, QuantityRole::Delta, _) => QuantityRole::Point,
            (QuantityRole::Delta, QuantityRole::Point, false) => {
                return other.checked_add(self, work);
            }
            _ => {
                return Err(RationalQuantityRefusal::Physical(
                    QuantityConversionRefusal::IncompatibleQuantityRoles,
                ))
            }
        };
        let other = other.convert_to_unit(self.unit, work)?;
        let coordinate = if subtract {
            self.coordinate.checked_subtract(other.coordinate, work)?
        } else {
            self.coordinate.checked_add(other.coordinate, work)?
        };
        Self::new(coordinate, self.unit, role)
    }
}
