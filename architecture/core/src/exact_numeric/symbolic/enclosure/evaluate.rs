//! Shared exact interval arithmetic for owning preparation and borrowed execution.
use super::*;
pub(super) fn node(
    node: Node,
    values: &[MathematicalInterval],
    requirements: MathematicalEnclosureRequirements,
    maximum_used_constant_terms: &mut u16,
    work: &mut ExactNumericWork,
) -> Result<MathematicalInterval, MathematicalEnclosureRefusal> {
    use MathematicalEnclosureRefusal as R;
    work.charge(1)?;
    let get = |id: super::super::MathematicalNodeId| {
        values
            .get(id.index as usize)
            .copied()
            .ok_or(R::InvalidExpression)
    };
    Ok(match node {
        Node::Rational(q) => MathematicalInterval::point(q),
        Node::Pi => {
            let (interval, terms) = pi::enclose(
                requirements.maximum_leaf_width,
                requirements.maximum_constant_terms,
                work,
            )?;
            *maximum_used_constant_terms = (*maximum_used_constant_terms).max(terms);
            interval
        }
        Node::SineTurns(_) | Node::CosineTurns(_) | Node::TangentTurns(_) => {
            return Err(R::UnsupportedTrigonometricEnclosure)
        }
        Node::ArctangentTurns(a) | Node::ArcsineTurns(a) | Node::ArccosineTurns(a) => {
            let (interval, terms) = inverse_node::enclose(node, get(a)?, requirements, work)?;
            *maximum_used_constant_terms = (*maximum_used_constant_terms).max(terms);
            interval
        }
        Node::Tau => return Err(R::InvalidExpression),
        Node::Negate(a) => get(a)?.negated(),
        Node::Add(a, b) => get(a)?.add(get(b)?, work)?,
        Node::Subtract(a, b) => get(a)?.subtract(get(b)?, work)?,
        Node::Multiply(a, b) => get(a)?.multiply(get(b)?, work)?,
        Node::Divide(a, b) => {
            let divisor = get(b)?;
            if divisor.lower() != divisor.upper() {
                get(a)?.multiply(divisor.reciprocal(work)?, work)?
            } else {
                get(a)?.scale(
                    ExactRational::ONE.checked_divide(divisor.lower(), work)?,
                    work,
                )?
            }
        }
        Node::PrincipalSquareRoot(a) => {
            let radicand = get(a)?;
            if radicand.lower() != radicand.upper() {
                return Err(R::InvalidExpression);
            }
            let root = PreparedSquareRoot::prepare(
                radicand.lower(),
                requirements.maximum_leaf_width,
                requirements.maximum_root_refinements,
                work,
            )?;
            MathematicalInterval::bounds(root.target().lower(), root.target().upper())
        }
    })
}
