//! Finite exact turn denotations. Non-cardinal functions remain symbolic.
use super::*;
impl MathematicalValueBuilder {
    pub(super) fn reduce_trigonometric_argument(
        &mut self,
        node: MathematicalNode,
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalNode, MathematicalValueRefusal> {
        use MathematicalNode as N;
        let argument = match node {
            N::SineTurns(a) | N::CosineTurns(a) | N::TangentTurns(a) => a,
            _ => return Ok(node),
        };
        let Some(q) = self.rational(argument) else {
            return Ok(node);
        };
        let reduced = self.insert(N::Rational(q.reduced_turn(work)?), work)?;
        Ok(match node {
            N::SineTurns(_) => N::SineTurns(reduced),
            N::CosineTurns(_) => N::CosineTurns(reduced),
            N::TangentTurns(_) => N::TangentTurns(reduced),
            _ => unreachable!(),
        })
    }
}
pub(super) fn cardinal(
    node: MathematicalNode,
    turns: ExactRational,
    work: &mut ExactNumericWork,
) -> Result<Option<ExactRational>, MathematicalValueRefusal> {
    use crate::AngleTrigonometricFunction as F;
    let function = match node {
        MathematicalNode::SineTurns(_) => F::Sine,
        MathematicalNode::CosineTurns(_) => F::Cosine,
        MathematicalNode::TangentTurns(_) => F::Tangent,
        _ => unreachable!(),
    };
    cardinal_function(function, turns, work)
}
fn cardinal_function(
    function: crate::AngleTrigonometricFunction,
    turns: ExactRational,
    work: &mut ExactNumericWork,
) -> Result<Option<ExactRational>, MathematicalValueRefusal> {
    use crate::AngleTrigonometricFunction as F;
    let Some(quadrant) = turns.quarter_turn_index(work)? else {
        return Ok(None);
    };
    let value = match function {
        F::Sine => match quadrant {
            1 => 1,
            3 => -1,
            _ => 0,
        },
        F::Cosine => match quadrant {
            0 => 1,
            2 => -1,
            _ => 0,
        },
        F::Tangent if quadrant == 1 || quadrant == 3 => {
            return Err(MathematicalValueRefusal::TrigonometricPole)
        }
        F::Tangent => 0,
    };
    Ok(Some(ExactRational::from_i128(value)))
}
/// Encode the canonical two-node unary graph directly into admitted storage.
/// The normal-form predicate is shared with builder and borrowed decoder.
pub(in crate::exact_numeric) fn encode_rational_turns(
    function: crate::AngleTrigonometricFunction,
    turns: ExactRational,
    output: &mut [u8],
    work: &mut ExactNumericWork,
) -> Result<usize, MathematicalValueRefusal> {
    use crate::AngleTrigonometricFunction as F;
    let turns = turns.reduced_turn(work)?;
    if let Some(value) = cardinal_function(function, turns, work)? {
        return MathematicalValue::rational(value).encode(output);
    }
    let small = || MathematicalValueRefusal::Numeric(ExactNumericRefusal::EncodingBufferTooSmall);
    if output.len() < 8 {
        return Err(small());
    }
    let length = turns.encode(&mut output[6..])?;
    if output.len() < 8 + length {
        return Err(small());
    }
    output[..4].copy_from_slice(&[1, 1, 2, 0]);
    output[4..6].copy_from_slice(&(length as u16).to_le_bytes());
    output[6 + length..8 + length].copy_from_slice(&[
        match function {
            F::Sine => 9,
            F::Cosine => 10,
            F::Tangent => 11,
        },
        0,
    ]);
    Ok(8 + length)
}
