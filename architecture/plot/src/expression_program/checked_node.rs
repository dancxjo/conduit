//! Convert one checked syntax tree into the existing typed program nodes.
use super::*;

pub(super) fn node(
    syntax: &ExpressionSyntax,
    checked: &CheckedExpression,
) -> Result<PortableExpressionNode, PortableExpressionProgramRefusal> {
    let value_type = checked
        .node_types
        .iter()
        .find(|node| same_span(node.span, syntax.span()))
        .ok_or(PortableExpressionProgramRefusal::MissingCheckedNodeType)?
        .value_type
        .structured_info_type_with(&checked.semantic_structures)?;
    let operation = match syntax {
        ExpressionSyntax::TypedGlyphLiteral(literal) => {
            let value = checked
                .glyph_values
                .resolve(literal)
                .and_then(crate::CanonicalStructuredStartupValue::try_concrete)
                .ok_or(PortableExpressionProgramRefusal::MissingCheckedNodeType)?;
            if value.value_type() != &value_type {
                return Err(PortableExpressionProgramRefusal::MalformedEncoding);
            }
            PortableExpressionOperation::Constant(value)
        }
        ExpressionSyntax::Input(_) => PortableExpressionOperation::Input,
        ExpressionSyntax::Atomic(value) => {
            if let Some(constant) = checked.immutable_constants.get(&value.text) {
                if constant.value_type() != &value_type {
                    return Err(PortableExpressionProgramRefusal::MalformedEncoding);
                }
                PortableExpressionOperation::Constant(constant.clone())
            } else if let Some(bytes) = checked
                .canonical_literals
                .get(&(value.span.start, value.span.end))
            {
                PortableExpressionOperation::CanonicalLiteral(bytes.clone())
            } else {
                PortableExpressionOperation::Literal(value.text.clone())
            }
        }
        ExpressionSyntax::Projection { value, member, .. }
            if matches!(
                value_type.shape(),
                conduit_core::StructuredInfoTypeShape::Variant { .. }
            ) && matches!(value.as_ref(), ExpressionSyntax::Atomic(_))
                && !checked
                    .node_types
                    .iter()
                    .any(|node| same_span(node.span, value.span())) =>
        {
            let ExpressionProjection::Field(case) = member else {
                return Err(PortableExpressionProgramRefusal::InvalidTupleIndex);
            };
            PortableExpressionOperation::Literal(case.text.clone())
        }
        ExpressionSyntax::Projection { value, member, .. } => {
            PortableExpressionOperation::Projection {
                value: Box::new(node(value, checked)?),
                member: match member {
                    ExpressionProjection::Field(field) => {
                        PortableExpressionProjection::Field(field.text.clone())
                    }
                    ExpressionProjection::TupleIndex(index) => {
                        PortableExpressionProjection::TupleIndex(
                            index
                                .text
                                .parse()
                                .map_err(|_| PortableExpressionProgramRefusal::InvalidTupleIndex)?,
                        )
                    }
                },
            }
        }
        ExpressionSyntax::Unary {
            operator, operand, ..
        } => PortableExpressionOperation::Unary {
            operator: *operator,
            operand: Box::new(node(operand, checked)?),
        },
        ExpressionSyntax::Binary {
            operator,
            left,
            right,
            ..
        } => PortableExpressionOperation::Binary {
            operator: *operator,
            proven: checked
                .proven_arithmetic
                .contains(&(syntax.span().start, syntax.span().end)),
            left: Box::new(node(left, checked)?),
            right: Box::new(node(right, checked)?),
        },
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => PortableExpressionOperation::Conditional {
            condition: Box::new(node(condition, checked)?),
            when_true: Box::new(node(when_true, checked)?),
            when_false: Box::new(node(when_false, checked)?),
        },
        ExpressionSyntax::Tuple { values, .. } => PortableExpressionOperation::Tuple(
            values
                .iter()
                .map(|value| node(value, checked))
                .collect::<Result<_, _>>()?,
        ),
        ExpressionSyntax::Record { fields, .. } => PortableExpressionOperation::Record(
            fields
                .iter()
                .map(|field| Ok((field.name.text.clone(), node(&field.value, checked)?)))
                .collect::<Result<_, PortableExpressionProgramRefusal>>()?,
        ),
        ExpressionSyntax::Collection { values, .. } => PortableExpressionOperation::Collection(
            values
                .iter()
                .map(|value| node(value, checked))
                .collect::<Result<_, _>>()?,
        ),
        ExpressionSyntax::Variant { tag, payload, .. } => PortableExpressionOperation::Variant {
            tag: tag.text.rsplit('.').next().expect("checked tag").into(),
            payload: Box::new(node(payload, checked)?),
        },
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => PortableExpressionOperation::SemanticCall {
            kind: kind.text.clone(),
            arguments: arguments
                .iter()
                .map(|argument| node(argument, checked))
                .collect::<Result<_, _>>()?,
        },
    };
    Ok(PortableExpressionNode {
        value_type,
        operation,
    })
}

pub(super) fn same_span(left: Span, right: Span) -> bool {
    left.start == right.start && left.end == right.end
}
