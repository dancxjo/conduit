//! Evaluation borrows admitted input and prepared result storage.
use super::*;

pub(super) fn evaluate_node<'a>(
    node: &'a mut PreparedNode,
    input: &'a [u8],
    input_kind: Option<PrimitiveInfoKind>,
) -> Result<PrimitiveValue<'a>, Refusal> {
    let expected = node.kind;
    let value = match &mut node.operation {
        PreparedOperation::InverseTrigonometry {
            function,
            left,
            right,
            output,
        } => {
            let a = evaluate_node(left, input, input_kind)?;
            let b = right
                .as_mut()
                .map(|b| evaluate_node(b, input, input_kind))
                .transpose()?;
            let n = crate::expression_inverse_trigonometry::encode(
                *function,
                a.as_slice(),
                b.as_ref().map(|v| v.as_slice()),
                output,
            )?;
            PrimitiveValue::borrowed(expected, &output[..n])?
        }
        PreparedOperation::AngleReference { argument, output } => {
            let value = evaluate_node(argument, input, input_kind)?;
            let length = crate::expression_angle_reference::encode(value.as_slice(), output)?;
            PrimitiveValue::borrowed(expected, &output[..length])?
        }
        PreparedOperation::Trigonometry {
            function,
            argument,
            output,
        } => {
            let value = evaluate_node(argument, input, input_kind)?;
            let length =
                crate::expression_trigonometry::encode(*function, value.as_slice(), output)?;
            PrimitiveValue::borrowed(expected, &output[..length])?
        }
        PreparedOperation::QuantityBinary {
            operator,
            left,
            right,
            output,
        } => {
            let left = evaluate_node(left, input, input_kind)?;
            let right = evaluate_node(right, input, input_kind)?;
            let encoded = crate::expression_quantity::evaluate(
                *operator,
                kind_name(left.kind),
                kind_name(right.kind),
                left.as_slice(),
                right.as_slice(),
            )
            .ok_or(Refusal::InvalidProgram)??;
            let bytes = encoded.as_slice();
            output[..bytes.len()].copy_from_slice(bytes);
            PrimitiveValue::borrowed(expected, &output[..bytes.len()])?
        }
        PreparedOperation::RationalBinary {
            operator,
            left,
            right,
            output,
        } => {
            let left = evaluate_node(left, input, input_kind)?;
            let right = evaluate_node(right, input, input_kind)?;
            let length = crate::mathematical_value::arithmetic(
                *operator,
                left.as_slice(),
                right.as_slice(),
                output,
            )?;
            PrimitiveValue::borrowed(expected, &output[..length])?
        }
        PreparedOperation::Equality(equality) => equality.evaluate(input)?,
        PreparedOperation::Input => {
            if Some(expected) != input_kind {
                return Err(Refusal::InvalidProgram);
            }
            PrimitiveValue::borrowed(expected, input)?
        }
        PreparedOperation::Literal(encoded) => PrimitiveValue::borrowed(expected, encoded)?,
        PreparedOperation::Unary { operator, operand } => {
            let operand = evaluate_node(operand, input, input_kind)?;
            evaluate_unary(*operator, expected, &operand)?
        }
        PreparedOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => {
            let left = evaluate_node(left, input, input_kind)?;
            let right = evaluate_node(right, input, input_kind)?;
            evaluate_binary(*operator, *proven, expected, &left, &right)?
        }
        PreparedOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let condition = evaluate_node(condition, input, input_kind)?;
            let selected = if decode_bool(&condition)? {
                when_true
            } else {
                when_false
            };
            evaluate_node(selected, input, input_kind)?
        }
        PreparedOperation::TextMaterial(operand) => evaluate_node(operand, input, input_kind)?,
        PreparedOperation::Widen(operand) => {
            let operand = evaluate_node(operand, input, input_kind)?;
            primitive::evaluate_widen(expected, &operand)?
        }
        PreparedOperation::Inspection(inspection) => inspection.evaluate(input)?,
        PreparedOperation::Bytes(observation) => observation.evaluate(input, input_kind)?,
        PreparedOperation::SequenceSelection(selection) => {
            PrimitiveValue::borrowed(expected, selection.evaluate(input)?)?
        }
        PreparedOperation::Projection(projection) => {
            PrimitiveValue::borrowed(expected, projection.evaluate(input)?)?
        }
    };
    if value.kind == expected {
        conduit_core::validate_primitive_info(&node.info_kind, value.as_slice())
            .map_err(|_| Refusal::InvalidProgram)?;
        Ok(value)
    } else {
        Err(Refusal::InvalidProgram)
    }
}
