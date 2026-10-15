//! Typed Angle admission and exact denotation, separate from numeric realization.
use super::*;
use crate::{
    ExactQuantityConversionRequestRefusal, Quantity, QuantityDimension, QuantityRole, Unit,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleTrigonometricFunction {
    Sine,
    Cosine,
    Tangent,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleTrigonometryRefusal {
    NotAngle,
    UnsupportedRole,
    SourceEvidence(ExactQuantityConversionRequestRefusal),
    Coordinate(RationalQuantityRefusal),
    Denotation(MathematicalValueRefusal),
}
/// Original spelling/Unit, exact coordinate transformation, and mathematical
/// result remain independently inspectable. This is not a machine projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAngleTrigonometry<'a> {
    function: AngleTrigonometricFunction,
    coordinates: RationalQuantityConversionReceipt<'a>,
    value: MathematicalValue,
}
impl<'a> PreparedAngleTrigonometry<'a> {
    pub fn from_checked(
        function: AngleTrigonometricFunction,
        source: Quantity,
        original: &'a str,
        work: &mut ExactNumericWork,
    ) -> Result<Self, AngleTrigonometryRefusal> {
        use AngleTrigonometryRefusal as R;
        if source.dimension() != QuantityDimension::Angle {
            return Err(R::NotAngle);
        }
        if source.role() != QuantityRole::Linear {
            return Err(R::UnsupportedRole);
        }
        let coordinates = RationalQuantityConversionReceipt::from_checked(
            source,
            Unit::Turn,
            original,
            "turn",
            work,
        )
        .map_err(R::SourceEvidence)?;
        let canonical = coordinates.result().map_err(R::Coordinate)?;
        let mut builder = MathematicalValueBuilder::new();
        let argument = builder
            .insert(MathematicalNode::Rational(canonical.coordinate()), work)
            .map_err(R::Denotation)?;
        let node = match function {
            AngleTrigonometricFunction::Sine => MathematicalNode::SineTurns(argument),
            AngleTrigonometricFunction::Cosine => MathematicalNode::CosineTurns(argument),
            AngleTrigonometricFunction::Tangent => MathematicalNode::TangentTurns(argument),
        };
        let root = builder.insert(node, work).map_err(R::Denotation)?;
        let value = builder.finish(root, work).map_err(R::Denotation)?;
        Ok(Self {
            function,
            coordinates,
            value,
        })
    }
    pub fn function(&self) -> AngleTrigonometricFunction {
        self.function
    }
    pub fn coordinate_projection(&self) -> &RationalQuantityConversionReceipt<'a> {
        &self.coordinates
    }
    pub fn denotation(&self) -> &MathematicalValue {
        &self.value
    }
}
