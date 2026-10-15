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
    use MathematicalNode as N;
    let Some(quadrant) = turns.quarter_turn_index(work)? else {
        return Ok(None);
    };
    let value = match node {
        N::SineTurns(_) => match quadrant {
            1 => 1,
            3 => -1,
            _ => 0,
        },
        N::CosineTurns(_) => match quadrant {
            0 => 1,
            2 => -1,
            _ => 0,
        },
        N::TangentTurns(_) if quadrant == 1 || quadrant == 3 => {
            return Err(MathematicalValueRefusal::TrigonometricPole)
        }
        N::TangentTurns(_) => 0,
        _ => unreachable!(),
    };
    Ok(Some(ExactRational::from_i128(value)))
}
