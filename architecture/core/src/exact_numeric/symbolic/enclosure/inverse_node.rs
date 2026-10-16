//! Project exact inverse denotations through the existing enclosure admission.
use super::*;
pub(super) fn enclose(
    node: Node,
    input: MathematicalInterval,
    requirements: MathematicalEnclosureRequirements,
    work: &mut ExactNumericWork,
) -> Result<(MathematicalInterval, u16), MathematicalEnclosureRefusal> {
    use MathematicalEnclosureRefusal as R;
    if input.lower() != input.upper() {
        return Err(R::InvalidExpression);
    }
    let leaf = requirements
        .maximum_leaf_width
        .checked_divide(ExactRational::from_i128(16), work)?;
    let angle = ArctangentTurnsRequirements {
        maximum_width_turns: requirements.maximum_leaf_width,
        maximum_series_width: leaf,
        maximum_terms: requirements.maximum_constant_terms,
    };
    if matches!(node, Node::ArctangentTurns(_)) {
        let prepared = PreparedArctangentTurns::prepare(input.lower(), angle, work)?;
        return Ok((prepared.target(), prepared.maximum_used_terms()));
    }
    let function = match node {
        Node::ArcsineTurns(_) => InverseCircularFunction::Arcsine,
        Node::ArccosineTurns(_) => InverseCircularFunction::Arccosine,
        _ => return Err(R::InvalidExpression),
    };
    let prepared = PreparedInverseCircular::prepare(
        function,
        input.lower(),
        InverseCircularRequirements {
            branch: InverseCircularBranch::Principal,
            angle,
            maximum_root_width: leaf,
            maximum_root_refinements: requirements.maximum_root_refinements,
        },
        work,
    )
    .map_err(|error| match error {
        InverseCircularRefusal::OutsideUnitDomain => R::InvalidExpression,
        InverseCircularRefusal::UnresolvedPositiveRoot => R::InverseRootResolution,
        InverseCircularRefusal::Enclosure(reason) => reason,
    })?;
    let terms = prepared.arctangent_bounds().map_or(0, |(a, b)| {
        a.maximum_used_terms().max(b.maximum_used_terms())
    });
    Ok((prepared.target(), terms))
}
