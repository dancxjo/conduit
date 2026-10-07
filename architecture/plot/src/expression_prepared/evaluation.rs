//! Bounded traversal over one admitted immutable input.
use super::*;

pub(super) fn evaluate_node<'a>(
    node: &'a mut PreparedNode,
    input: &'a [u8],
    input_kind: Option<PrimitiveInfoKind>,
    canonical: EvaluationInput<'a>,
) -> Result<PrimitiveValue<'a>, Refusal> {
    let expected = node.kind;
    let value = match &mut node.operation {
        PreparedOperation::Equality(equality) => equality.evaluate(canonical)?,
        PreparedOperation::Input => {
            if Some(expected) != input_kind {
                return Err(Refusal::InvalidProgram);
            }
            PrimitiveValue::borrowed(expected, input)?
        }
        PreparedOperation::Literal(encoded) => PrimitiveValue::borrowed(expected, encoded)?,
        PreparedOperation::Unary { operator, operand } => {
            let operand = evaluate_node(operand, input, input_kind, canonical)?;
            evaluate_unary(*operator, expected, &operand)?
        }
        PreparedOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => {
            let left = evaluate_node(left, input, input_kind, canonical)?;
            let right = evaluate_node(right, input, input_kind, canonical)?;
            evaluate_binary(*operator, *proven, expected, &left, &right)?
        }
        PreparedOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let condition = evaluate_node(condition, input, input_kind, canonical)?;
            let selected = if decode_bool(&condition)? {
                when_true
            } else {
                when_false
            };
            evaluate_node(selected, input, input_kind, canonical)?
        }
        PreparedOperation::TextMaterial(operand) => {
            evaluate_node(operand, input, input_kind, canonical)?
        }
        PreparedOperation::Widen(operand) => {
            let operand = evaluate_node(operand, input, input_kind, canonical)?;
            primitive::evaluate_widen(expected, &operand)?
        }
        PreparedOperation::Inspection(inspection) => inspection.evaluate(canonical)?,
        PreparedOperation::Bytes(observation) => {
            observation.evaluate(input, input_kind, canonical)?
        }
        PreparedOperation::SequenceSelection(selection) => {
            PrimitiveValue::borrowed(expected, selection.evaluate(canonical)?)?
        }
        PreparedOperation::Projection(projection) => {
            PrimitiveValue::borrowed(expected, projection.evaluate(canonical)?)?
        }
    };
    if value.kind == expected {
        Ok(value)
    } else {
        Err(Refusal::InvalidProgram)
    }
}
