//! Numeric and Boolean operator typing; no construction or name resolution.
use super::*;
use crate::expression_numeric_type::{
    is_fixed_integer, is_numeric, is_ordered_numeric, is_signed_numeric,
};

pub(super) fn unary(
    operator: UnaryOperator,
    operand: &ExpressionSyntax,
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    match operator {
        UnaryOperator::Not => {
            let actual = infer(operand, Some(&boolean()), context, node_types)?;
            require(actual, &boolean(), span, "! requires Boolean")?;
            Ok(boolean())
        }
        UnaryOperator::Negate => {
            let actual = infer(operand, expected, context, node_types)?;
            if is_signed_numeric(&actual, context) {
                Ok(actual)
            } else {
                refuse(span, "unary - requires an exact signed numeric type")
            }
        }
    }
}

pub(super) fn binary(
    operator: BinaryOperator,
    left: &ExpressionSyntax,
    right: &ExpressionSyntax,
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    if matches!(
        operator,
        BinaryOperator::BooleanAnd | BinaryOperator::BooleanOr
    ) {
        let expected = boolean();
        require(
            infer(left, Some(&expected), context, node_types)?,
            &expected,
            left.span(),
            "Boolean operator requires Boolean operands",
        )?;
        require(
            infer(right, Some(&expected), context, node_types)?,
            &expected,
            right.span(),
            "Boolean operator requires Boolean operands",
        )?;
        return Ok(expected);
    }
    let checkpoint = node_types.len();
    let left_type = match infer(left, expected, context, node_types) {
        Ok(value_type) => value_type,
        Err(left_error) => {
            node_types.truncate(checkpoint);
            match infer(right, expected, context, node_types) {
                Ok(right_type) => {
                    infer(left, Some(&right_type), context, node_types).map_err(|_| left_error)?
                }
                Err(_) => return Err(left_error),
            }
        }
    };
    let right_type = infer(right, Some(&left_type), context, node_types)?;
    require(
        right_type,
        &left_type,
        span,
        "binary operands must have one exact type",
    )?;
    match operator {
        BinaryOperator::Equal | BinaryOperator::NotEqual => Ok(boolean()),
        BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
            if is_ordered_numeric(&left_type, context) =>
        {
            Ok(boolean())
        }
        BinaryOperator::BitAnd | BinaryOperator::BitXor | BinaryOperator::BitOr
            if is_fixed_integer(&left_type, context) =>
        {
            Ok(left_type)
        }
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            if is_fixed_integer(&left_type, context) =>
        {
            Ok(left_type)
        }
        BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder
        | BinaryOperator::Add
        | BinaryOperator::Subtract
            if is_numeric(&left_type, context) =>
        {
            Ok(left_type
                .value_kind()
                .and_then(|kind| context.structured_types.get(kind))
                .and_then(|value_type| match value_type.shape() {
                    StructuredInfoTypeShape::Nominal { representation, .. } => {
                        match representation.shape() {
                            StructuredInfoTypeShape::Leaf(kind) => {
                                Some(CheckedExpressionType::Semantic(kind.clone()))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .unwrap_or(left_type))
        }
        _ => refuse(span, "operator is not defined for this exact type"),
    }
}
