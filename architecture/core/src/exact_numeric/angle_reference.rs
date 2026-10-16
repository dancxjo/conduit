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
        validate_angle(source, target_unit)?;
        let (source_suffix, target) = crate::quantity::checked_conversion_evidence(
            source,
            target_unit,
            original,
            target_source,
        )
        .map_err(R::SourceEvidence)?;
        let (turns, coefficient) = radian_coefficient(source, target_unit, work)?;
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

fn validate_angle(source: Quantity, target: Unit) -> Result<(), AngleReferenceRefusal> {
    if source.dimension() != QuantityDimension::Angle
        || target.dimension() != Unit::Radian.dimension()
        || source.role() != QuantityRole::Linear
        || !target.admits_role(QuantityRole::Linear)
    {
        return Err(AngleReferenceRefusal::NotLinearAngle);
    }
    Ok(())
}
fn radian_coefficient(
    source: Quantity,
    target: Unit,
    work: &mut ExactNumericWork,
) -> Result<(RationalQuantity, ExactRational), AngleReferenceRefusal> {
    use AngleReferenceRefusal as R;
    validate_angle(source, target)?;
    let turns = RationalQuantity::from_quantity(source, work)
        .and_then(|q| q.convert_to_unit(Unit::Turn, work))
        .map_err(R::Coordinate)?;
    let radians = RationalQuantity::new(ExactRational::ONE, Unit::Radian, QuantityRole::Linear)
        .and_then(|q| q.convert_to_unit(target, work))
        .map_err(R::Coordinate)?;
    let coefficient = turns
        .coordinate()
        .checked_multiply(ExactRational::from_i128(2), work)
        .and_then(|q| q.checked_multiply(radians.coordinate(), work))
        .map_err(|e| R::Denotation(e.into()))?;
    Ok((turns, coefficient))
}

/// Bounded writer for the same symbolic law; runtime storage is caller-owned.
/// The containing Source/receipt retains the original Angle and target Unit.
pub fn encode_angle_radian_denotation(
    source: Quantity,
    target: Unit,
    output: &mut [u8],
    work: &mut ExactNumericWork,
) -> Result<usize, AngleReferenceRefusal> {
    use AngleReferenceRefusal as R;
    let (_, coefficient) = radian_coefficient(source, target, work)?;
    if coefficient == ExactRational::ZERO {
        return MathematicalValue::rational(coefficient)
            .encode(output)
            .map_err(R::Denotation);
    }
    let small = || R::Denotation(ExactNumericRefusal::EncodingBufferTooSmall.into());
    if coefficient == ExactRational::ONE {
        if output.len() < 4 {
            return Err(small());
        }
        output[..4].copy_from_slice(&[1, 1, 1, 1]);
        return Ok(4);
    }
    if output.len() < 10 {
        return Err(small());
    }
    let length = coefficient
        .encode(&mut output[6..])
        .map_err(|e| R::Denotation(e.into()))?;
    if output.len() < 10 + length {
        return Err(small());
    }
    output[..4].copy_from_slice(&[1, 1, 3, 0]);
    output[4..6].copy_from_slice(&(length as u16).to_le_bytes());
    output[6 + length..10 + length].copy_from_slice(&[1, 6, 0, 1]);
    Ok(10 + length)
}
