//! Exact Angle denotation precedes numerical profile selection.
use super::typed_atan2::{Atan2Branch, Atan2QuantityInputs, Atan2Refusal as R};
use crate::{
    ExactNumericWork as Work, ExactRational as Q, MathematicalNode as N, MathematicalValue,
    MathematicalValueBuilder, QuantityRole, RationalQuantityConversionReceipt, Unit,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantityAtan2Denotation<'a> {
    y: RationalQuantityConversionReceipt<'a>,
    x: RationalQuantityConversionReceipt<'a>,
    coordinate: MathematicalValue,
    branch: Atan2Branch,
}
impl<'a> QuantityAtan2Denotation<'a> {
    pub fn from_checked(
        inputs: Atan2QuantityInputs<'a>,
        branch: Atan2Branch,
        work: &mut Work,
    ) -> Result<Self, R> {
        if inputs.y.dimension() != inputs.x.dimension()
            || inputs.y.dimension() != inputs.common_unit.dimension()
        {
            return Err(R::MismatchedDimensions);
        }
        if inputs.y.role() != inputs.x.role()
            || !matches!(inputs.y.role(), QuantityRole::Linear | QuantityRole::Delta)
        {
            return Err(R::UnsupportedRole);
        }
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
        let yq = y.result().map_err(R::Coordinate)?.coordinate();
        let xq = x.result().map_err(R::Coordinate)?.coordinate();
        if yq == Q::ZERO && xq == Q::ZERO {
            return Err(R::UndefinedZeroPair);
        }
        let mut builder = MathematicalValueBuilder::new();
        let root = if xq == Q::ZERO {
            let quarter = Q::ratio(1, 4, work)?;
            builder.insert(
                N::Rational(if yq.is_negative() {
                    quarter.negated()
                } else {
                    quarter
                }),
                work,
            )?
        } else {
            let ratio = builder.insert(N::Rational(yq.checked_divide(xq, work)?), work)?;
            let atan = builder.insert(N::ArctangentTurns(ratio), work)?;
            if xq.is_negative() {
                let half = Q::ratio(1, 2, work)?;
                let offset = builder.insert(
                    N::Rational(if yq.is_negative() {
                        half.negated()
                    } else {
                        half
                    }),
                    work,
                )?;
                builder.insert(N::Add(atan, offset), work)?
            } else {
                atan
            }
        };
        Ok(Self {
            y,
            x,
            coordinate: builder.finish(root, work)?,
            branch,
        })
    }
    pub const fn y_conversion(&self) -> RationalQuantityConversionReceipt<'a> {
        self.y
    }
    pub const fn x_conversion(&self) -> RationalQuantityConversionReceipt<'a> {
        self.x
    }
    pub fn coordinate(&self) -> &MathematicalValue {
        &self.coordinate
    }
    pub const fn unit(&self) -> Unit {
        Unit::Turn
    }
    pub const fn branch(&self) -> Atan2Branch {
        self.branch
    }
}
