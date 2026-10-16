//! Source-correlated same-Dimension atan2 with an explicit common Unit.
use super::{
    ArctangentTurnsRequirements, MathematicalEnclosureRefusal, MathematicalInterval as Interval,
    PreparedArctangentTurns,
};
use crate::{
    ExactNumericRefusal, ExactNumericWork as Work, ExactQuantityConversionRequestRefusal,
    ExactRational as Q, Quantity, QuantityRole, RationalQuantity,
    RationalQuantityConversionReceipt, RationalQuantityRefusal, Unit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atan2Branch {
    /// (-1/2, 1/2] turns; the exact negative x-axis selects +1/2.
    PrincipalSignedHalfTurn,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Atan2QuantityInputs<'a> {
    pub y: Quantity,
    pub original_y: &'a str,
    pub x: Quantity,
    pub original_x: &'a str,
    pub common_unit: Unit,
    pub common_unit_source: &'a str,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Atan2Requirements {
    pub branch: Atan2Branch,
    pub numeric: ArctangentTurnsRequirements,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atan2Refusal {
    MismatchedDimensions,
    UnsupportedRole,
    UndefinedZeroPair,
    SourceEvidence(ExactQuantityConversionRequestRefusal),
    Coordinate(RationalQuantityRefusal),
    Enclosure(MathematicalEnclosureRefusal),
}
impl From<ExactNumericRefusal> for Atan2Refusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Enclosure(MathematicalEnclosureRefusal::Numeric(value))
    }
}
impl From<MathematicalEnclosureRefusal> for Atan2Refusal {
    fn from(value: MathematicalEnclosureRefusal) -> Self {
        Self::Enclosure(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedQuantityAtan2<'a> {
    y: RationalQuantityConversionReceipt<'a>,
    x: RationalQuantityConversionReceipt<'a>,
    lower: RationalQuantity,
    upper: RationalQuantity,
    width_turns: Q,
    requirements: Atan2Requirements,
    limb_work: u64,
}
impl<'a> PreparedQuantityAtan2<'a> {
    pub fn from_checked(
        inputs: Atan2QuantityInputs<'a>,
        requirements: Atan2Requirements,
        work: &mut Work,
    ) -> Result<Self, Atan2Refusal> {
        use Atan2Refusal as R;
        if inputs.y.dimension() != inputs.x.dimension()
            || inputs.y.dimension() != inputs.common_unit.dimension()
        {
            return Err(R::MismatchedDimensions);
        }
        if inputs.y.role() != QuantityRole::Linear || inputs.x.role() != QuantityRole::Linear {
            return Err(R::UnsupportedRole);
        }
        let before = work.spent();
        let convert = |value, original, work: &mut Work| {
            RationalQuantityConversionReceipt::from_checked(
                value,
                inputs.common_unit,
                original,
                inputs.common_unit_source,
                work,
            )
            .map_err(R::SourceEvidence)
        };
        let y = convert(inputs.y, inputs.original_y, work)?;
        let x = convert(inputs.x, inputs.original_x, work)?;
        let y_coordinate = y.result().map_err(R::Coordinate)?.coordinate();
        let x_coordinate = x.result().map_err(R::Coordinate)?.coordinate();
        if y_coordinate == Q::ZERO && x_coordinate == Q::ZERO {
            return Err(R::UndefinedZeroPair);
        }
        let ratio = if x_coordinate == Q::ZERO {
            Q::ZERO
        } else {
            y_coordinate.checked_divide(x_coordinate, work)?
        };
        let atan = PreparedArctangentTurns::prepare(ratio, requirements.numeric, work)
            .map_err(R::Enclosure)?;
        let half = Q::ratio(1, 2, work)?;
        let quarter = Q::ratio(1, 4, work)?;
        let target = if x_coordinate == Q::ZERO {
            Interval::point(if y_coordinate.is_negative() {
                quarter.negated()
            } else {
                quarter
            })
        } else if x_coordinate.is_negative() {
            atan.target().add(
                Interval::point(if y_coordinate.is_negative() {
                    half.negated()
                } else {
                    half
                }),
                work,
            )?
        } else {
            atan.target()
        };
        let width_turns = target.width(work)?;
        let angle = |coordinate| {
            RationalQuantity::new(coordinate, Unit::Turn, QuantityRole::Linear)
                .map_err(R::Coordinate)
        };
        Ok(Self {
            y,
            x,
            lower: angle(target.lower())?,
            upper: angle(target.upper())?,
            width_turns,
            requirements,
            limb_work: work.spent() - before,
        })
    }
    pub const fn y_conversion(&self) -> RationalQuantityConversionReceipt<'a> {
        self.y
    }
    pub const fn x_conversion(&self) -> RationalQuantityConversionReceipt<'a> {
        self.x
    }
    pub const fn lower(&self) -> RationalQuantity {
        self.lower
    }
    pub const fn upper(&self) -> RationalQuantity {
        self.upper
    }
    pub const fn width_turns(&self) -> Q {
        self.width_turns
    }
    pub const fn requirements(&self) -> Atan2Requirements {
        self.requirements
    }
    pub const fn limb_work(&self) -> u64 {
        self.limb_work
    }
}
