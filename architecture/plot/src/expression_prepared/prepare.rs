//! Prepare typed expression nodes and admit all result storage before Play.
use super::*;

pub(super) fn prepare_node(
    node: &PortableExpressionNode,
    input_type: &conduit_core::StructuredInfoType,
    prepared_input: &PreparedInput,
) -> Result<PreparedNode, Refusal> {
    let kind = leaf_kind(&node.value_type)?;
    let operation = match &node.operation {
        PortableExpressionOperation::Constant(value) => {
            if value.value_type() != &node.value_type {
                return Err(Refusal::InvalidProgram);
            }
            let conduit_core::StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                return Err(Refusal::InvalidProgram);
            };
            PreparedOperation::Literal(bytes.to_vec())
        }
        PortableExpressionOperation::Input => PreparedOperation::Input,
        PortableExpressionOperation::Literal(literal) => PreparedOperation::Literal(
            crate::expression_evaluate::literal_primitive_bytes(&node.value_type, literal)?,
        ),
        PortableExpressionOperation::CanonicalLiteral(encoded) => {
            crate::expression_program::validate_capsule_literal(&node.value_type, encoded)
                .map_err(|_| Refusal::InvalidProgram)?;
            PreparedOperation::Literal(encoded.clone())
        }
        PortableExpressionOperation::Unary { operator, operand } => PreparedOperation::Unary {
            operator: *operator,
            operand: Box::new(prepare_node(operand, input_type, prepared_input)?),
        },
        PortableExpressionOperation::Binary {
            operator,
            left,
            right,
            ..
        } if matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Remainder
        ) && [left.as_ref(), right.as_ref()].iter().any(|node| {
            leaf_info_kind(&node.value_type).is_ok_and(crate::expression_quantity::is_quantity)
        }) =>
        {
            PreparedOperation::QuantityBinary {
                profile: prepared_input.quantity_profile(),
                operator: *operator,
                left: Box::new(prepare_node(left, input_type, prepared_input)?),
                right: Box::new(prepare_node(right, input_type, prepared_input)?),
                output: vec![0; prepared_input.quantity_profile().maximum_encoded_bytes()],
            }
        }
        PortableExpressionOperation::Binary {
            operator,
            left,
            right,
            ..
        } if kind == PrimitiveInfoKind::MathematicalValue
            && matches!(
                operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
            ) =>
        {
            PreparedOperation::MathematicalBinary {
                writer: Box::new(crate::mathematical_value::PreparedArithmetic::new()?),
                operator: *operator,
                left: Box::new(prepare_node(left, input_type, prepared_input)?),
                right: Box::new(prepare_node(right, input_type, prepared_input)?),
                output: vec![0; conduit_core::MATHEMATICAL_VALUE_MAX_ENCODED_BYTES],
            }
        }
        PortableExpressionOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => equality::binary(
            *operator,
            *proven,
            kind,
            left,
            right,
            input_type,
            prepared_input,
        )?,
        PortableExpressionOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => PreparedOperation::Conditional {
            condition: Box::new(prepare_node(condition, input_type, prepared_input)?),
            when_true: Box::new(prepare_node(when_true, input_type, prepared_input)?),
            when_false: Box::new(prepare_node(when_false, input_type, prepared_input)?),
        },
        PortableExpressionOperation::SemanticCall { kind, arguments }
            if kind == crate::expression_square_root::CALL =>
        {
            square_root::prepare(node, arguments, input_type, prepared_input)?
        }
        PortableExpressionOperation::SemanticCall { kind, arguments }
            if crate::expression_inverse_trigonometry::function(kind).is_some() =>
        {
            inverse_trigonometry::prepare(node, kind, arguments, input_type, prepared_input)?
        }
        PortableExpressionOperation::SemanticCall { kind, arguments }
            if kind == crate::expression_angle_reference::CALL =>
        {
            angle_reference::prepare(node, arguments, input_type, prepared_input)?
        }
        PortableExpressionOperation::SemanticCall { kind, arguments }
            if crate::expression_trigonometry::function(kind).is_some() =>
        {
            trigonometry::prepare(node, kind, arguments, input_type, prepared_input)?
        }
        PortableExpressionOperation::SemanticCall { kind, arguments }
            if kind == "text/material" =>
        {
            text_material::prepare(node, arguments, input_type, prepared_input)?
        }
        PortableExpressionOperation::SemanticCall {
            kind: call,
            arguments,
        } if crate::expression_semantic_call::integer_widening_target(call).is_some() => {
            let [argument] = arguments.as_slice() else {
                return Err(Refusal::InvalidProgram);
            };
            let operand = prepare_node(argument, input_type, prepared_input)?;
            if call != kind_name(kind)
                || !crate::expression_semantic_call::is_strict_widening(
                    kind_name(operand.kind),
                    call,
                )
            {
                return Err(Refusal::InvalidProgram);
            }
            PreparedOperation::Widen(Box::new(operand))
        }
        PortableExpressionOperation::SemanticCall {
            kind: call,
            arguments,
        } if matches!(
            call.as_str(),
            "variant/is" | "variant/tag" | "sequence/length"
        ) =>
        {
            let expected = match call.as_str() {
                "variant/is" => PrimitiveInfoKind::Bool,
                "variant/tag" => PrimitiveInfoKind::Text,
                _ => PrimitiveInfoKind::U64,
            };
            if kind != expected {
                return Err(Refusal::InvalidProgram);
            }
            PreparedOperation::Inspection(inspection::PreparedInspection::new(
                call,
                arguments,
                input_type,
                prepared_input,
            )?)
        }
        PortableExpressionOperation::SemanticCall {
            kind: call,
            arguments,
        } if matches!(call.as_str(), "bytes/length" | "bytes/at" | "bytes/window") => {
            let expected = if call == "bytes/window" {
                PrimitiveInfoKind::Bytes
            } else if call == "bytes/at" {
                PrimitiveInfoKind::U8
            } else {
                PrimitiveInfoKind::U64
            };
            if kind != expected {
                return Err(Refusal::InvalidProgram);
            }
            PreparedOperation::Bytes(byte_observation::PreparedByteObservation::new(
                call,
                arguments,
                input_type,
                prepared_input,
            )?)
        }
        PortableExpressionOperation::SemanticCall { kind: call, .. } if call == "sequence/at" => {
            PreparedOperation::SequenceSelection(
                sequence_selection::PreparedSequenceSelection::new(
                    node,
                    input_type,
                    prepared_input,
                )?,
            )
        }
        PortableExpressionOperation::Projection { .. } => PreparedOperation::Projection(
            member_selection::prepare(node, input_type, prepared_input)?,
        ),
        _ => {
            return Err(Refusal::UnsupportedType(
                "structured expression runtime".into(),
            ));
        }
    };
    Ok(PreparedNode {
        quantity_profile: prepared_input.quantity_profile(),
        kind,
        info_kind: leaf_info_kind(&node.value_type)?.into(),
        operation,
    })
}
