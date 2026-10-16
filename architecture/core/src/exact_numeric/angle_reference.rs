//! Explicit symbolic bridge between the independent turn and radian origins.
use super::*;
use crate::{
    ExactQuantityConversionRequestRefusal, Quantity, QuantityDimension, QuantityRole,
    ResolvedQuantitySuffix, Unit,
};

pub const TURN_RADIAN_REFERENCE_LAW: &str = "physical/angle-turn-radian-2pi@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleReferenceRefusal {
    NotLinearAngle,
    SourceEvidence(ExactQuantityConversionRequestRefusal),
    Coordinate(RationalQuantityRefusal),
    Denotation(MathematicalValueRefusal),
}

/// Reference conversion retains exact source bytes and Unit identities. The
/// mathematical target is not a rational Unit scale or a machine approximation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAngleReferenceConversion<'a> {
    original: &'a str,
    source: Quantity,
    source_suffix: ResolvedQuantitySuffix<'a>,
    target: ResolvedQuantitySuffix<'a>,
    turns: RationalQuantity,
    value: MathematicalValue,
}

impl<'a> PreparedAngleReferenceConversion<'a> {
    /// First profile: a turn-origin Angle to a radian-origin Unit, including
    /// exact declared scales/prefixes. Other origin bridges remain unsupported.
    pub fn from_checked(
        source: Quantity,
        target_unit: Unit,
        original: &'a str,
        target_source: &'a str,
        work: &mut ExactNumericWork,
    ) -> Result<Self, AngleReferenceRefusal> {
        use AngleReferenceRefusal as R;
        if source.dimension() != QuantityDimension::Angle
            || target_unit.dimension() != Unit::Radian.dimension()
            || source.role() != QuantityRole::Linear
            || !target_unit.admits_role(QuantityRole::Linear)
        {
            return Err(R::NotLinearAngle);
        }
        let (source_suffix, target) = crate::quantity::checked_conversion_evidence(
            source,
            target_unit,
            original,
            target_source,
        )
        .map_err(R::SourceEvidence)?;
        let turns = RationalQuantity::from_quantity(source, work)
            .and_then(|q| q.convert_to_unit(Unit::Turn, work))
            .map_err(R::Coordinate)?;
        let radians = RationalQuantity::new(ExactRational::ONE, Unit::Radian, QuantityRole::Linear)
            .and_then(|q| q.convert_to_unit(target_unit, work))
            .map_err(R::Coordinate)?;
        let coefficient = turns
            .coordinate()
            .checked_multiply(ExactRational::from_i128(2), work)
            .and_then(|q| q.checked_multiply(radians.coordinate(), work))
            .map_err(|e| R::Denotation(e.into()))?;
        let value = if coefficient == ExactRational::ZERO {
            MathematicalValue::rational(ExactRational::ZERO)
        } else {
            let mut builder = MathematicalValueBuilder::new();
            let pi = builder
                .insert(MathematicalNode::Pi, work)
                .map_err(R::Denotation)?;
            let root = if coefficient == ExactRational::ONE {
                pi
            } else {
                let scale = builder
                    .insert(MathematicalNode::Rational(coefficient), work)
                    .map_err(R::Denotation)?;
                builder
                    .insert(MathematicalNode::Multiply(scale, pi), work)
                    .map_err(R::Denotation)?
            };
            builder.finish(root, work).map_err(R::Denotation)?
        };
        Ok(Self {
            original,
            source,
            source_suffix,
            target,
            turns,
            value,
        })
    }
    pub const fn original(&self) -> &'a str {
        self.original
    }
    pub const fn source(&self) -> Quantity {
        self.source
    }
    pub const fn source_suffix(&self) -> ResolvedQuantitySuffix<'a> {
        self.source_suffix
    }
    pub const fn target(&self) -> ResolvedQuantitySuffix<'a> {
        self.target
    }
    pub const fn turns(&self) -> RationalQuantity {
        self.turns
    }
    pub const fn reference_law(&self) -> &'static str {
        TURN_RADIAN_REFERENCE_LAW
    }
    pub fn denotation(&self) -> &MathematicalValue {
        &self.value
    }
}
