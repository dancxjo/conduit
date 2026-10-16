//! Source-correlated same-Dimension atan2 with an explicit common Unit.
use super::{
    ArctangentTurnsRequirements, MathematicalEnclosureRefusal, MathematicalEnclosureRequirements,
    PreparedArctangentTurns, PreparedMathematicalEnclosure, QuantityAtan2Denotation,
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
    Denotation(crate::MathematicalValueRefusal),
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

impl From<crate::MathematicalValueRefusal> for Atan2Refusal {
    fn from(value: crate::MathematicalValueRefusal) -> Self {
        Self::Denotation(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedQuantityAtan2<'a> {
    source: QuantityAtan2Denotation<'a>,
    projection: PreparedMathematicalEnclosure,
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
        let before = work.spent();
        let source = QuantityAtan2Denotation::from_checked(inputs, requirements.branch, work)?;
        PreparedArctangentTurns::prepare(Q::ZERO, requirements.numeric, work)
            .map_err(R::Enclosure)?;
        let series_leaf = requirements
            .numeric
            .maximum_series_width
            .checked_multiply(Q::from_i128(16), work)?;
        let maximum_leaf_width = if series_leaf
            .checked_compare(requirements.numeric.maximum_width_turns, work)?
            == core::cmp::Ordering::Less
        {
            series_leaf
        } else {
            requirements.numeric.maximum_width_turns
        };
        // This is bounded preparation storage. Host admission separately
        // accounts for stack frames and allocator overhead, as for the ordinary enclosure entrance.
        let projection =
            PreparedMathematicalEnclosure::prepare(
                source.coordinate(),
                MathematicalEnclosureRequirements {
                    maximum_width: requirements.numeric.maximum_width_turns,
                    maximum_leaf_width,
                    maximum_constant_terms: requirements.numeric.maximum_terms,
                    maximum_root_refinements: 0,
                    maximum_scratch_payload_bytes: (crate::MATHEMATICAL_VALUE_MAX_NODES
                        * core::mem::size_of::<super::MathematicalInterval>())
                        as u32,
                    maximum_retained_payload_bytes: (core::mem::size_of::<
                        PreparedMathematicalEnclosure,
                    >() + crate::MATHEMATICAL_VALUE_MAX_NODES
                        * core::mem::size_of::<crate::MathematicalNode>())
                        as u32,
                },
                work,
            )
            .map_err(R::Enclosure)?;
        let target = projection.target();
        let width_turns = projection.width();
        let angle = |coordinate| {
            RationalQuantity::new(coordinate, Unit::Turn, QuantityRole::Linear)
                .map_err(R::Coordinate)
        };
        Ok(Self {
            source,
            projection,
            lower: angle(target.lower())?,
            upper: angle(target.upper())?,
            width_turns,
            requirements,
            limb_work: work.spent() - before,
        })
    }
    pub fn denotation(&self) -> &QuantityAtan2Denotation<'a> {
        &self.source
    }
    pub fn projection(&self) -> &PreparedMathematicalEnclosure {
        &self.projection
    }
    pub const fn y_conversion(&self) -> RationalQuantityConversionReceipt<'a> {
        self.source.y_conversion()
    }
    pub const fn x_conversion(&self) -> RationalQuantityConversionReceipt<'a> {
        self.source.x_conversion()
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
