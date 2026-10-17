//! Exact inverse denotation writers over caller-admitted runtime storage.
use super::{mathematical_angle, symbolic};
use crate::{
    Atan2Refusal, ExactNumericRefusal, ExactNumericWork as Work, ExactRational as Q,
    MathematicalAngleRefusal, MathematicalValueRefusal, Quantity, QuantityRole, RationalQuantity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseAngleFunction {
    Arctangent,
    Arcsine,
    Arccosine,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseAngleEncodingRefusal {
    Quantity(Atan2Refusal),
    Denotation(MathematicalValueRefusal),
    Angle(MathematicalAngleRefusal),
}
impl From<ExactNumericRefusal> for InverseAngleEncodingRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Denotation(value.into())
    }
}
impl From<MathematicalValueRefusal> for InverseAngleEncodingRefusal {
    fn from(value: MathematicalValueRefusal) -> Self {
        Self::Denotation(value)
    }
}
/// Maximum graph: two rational nodes, inverse node, and exact quadrant offset.
pub const INVERSE_ANGLE_MAX_ENCODED_BYTES: usize =
    1 + crate::UNIT_ENCODED_LEN + 3 + 2 * (3 + crate::EXACT_RATIONAL_MAX_ENCODED_BYTES) + 2 + 3;

pub fn encode_inverse_angle_denotation(
    function: InverseAngleFunction,
    input: Q,
    output: &mut [u8],
    work: &mut Work,
) -> Result<usize, InverseAngleEncodingRefusal> {
    work.charge(1)?;
    let prefix =
        mathematical_angle::write_prefix(output).map_err(InverseAngleEncodingRefusal::Angle)?;
    let length = symbolic::encode_inverse_coordinate(function, input, &mut output[prefix..], work)?;
    Ok(prefix + length)
}
/// Input Quantity encodings already retain their immutable Unit descriptors.
/// Conversion uses x's exact Unit law; neither input is assumed to be radians.
pub fn encode_quantity_atan2_denotation(
    y: Quantity,
    x: Quantity,
    output: &mut [u8],
    work: &mut Work,
) -> Result<usize, InverseAngleEncodingRefusal> {
    use InverseAngleEncodingRefusal as R;
    work.charge(1)?;
    if y.dimension() != x.dimension() {
        return Err(R::Quantity(Atan2Refusal::MismatchedDimensions));
    }
    if y.role() != QuantityRole::Linear || x.role() != QuantityRole::Linear {
        return Err(R::Quantity(Atan2Refusal::UnsupportedRole));
    }
    let coordinate = |value, work: &mut Work| {
        RationalQuantity::from_quantity(value, work)
            .and_then(|q| q.convert_to_unit(x.unit(), work))
            .map(|q| q.coordinate())
            .map_err(|e| R::Quantity(Atan2Refusal::Coordinate(e)))
    };
    let y = coordinate(y, work)?;
    let x = coordinate(x, work)?;
    if y == Q::ZERO && x == Q::ZERO {
        return Err(R::Quantity(Atan2Refusal::UndefinedZeroPair));
    }
    work.charge(1)?;
    let prefix = mathematical_angle::write_prefix(output).map_err(R::Angle)?;
    let length = if x == Q::ZERO {
        let quarter = Q::ratio(1, 4, work)?;
        crate::MathematicalValue::rational(if y.is_negative() {
            quarter.negated()
        } else {
            quarter
        })
        .encode(&mut output[prefix..])?
    } else {
        let ratio = y.checked_divide(x, work)?;
        let offset = if x.is_negative() {
            let half = Q::ratio(1, 2, work)?;
            if y.is_negative() {
                half.negated()
            } else {
                half
            }
        } else {
            Q::ZERO
        };
        symbolic::encode_shifted_arctangent(ratio, offset, &mut output[prefix..], work)?
    };
    Ok(prefix + length)
}
