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
    if (is_quantity(context.input)
        || expected.is_some_and(is_quantity)
        || physical_literal(left)
        || physical_literal(right))
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Remainder
        )
    {
        let left_type = infer_speculative(left, None, context, node_types);
        let right_type = infer_speculative(right, None, context, node_types);
        let scalar = CheckedExpressionType::semantic(conduit_core::SCALAR_INFO_ID);
        let left_type = left_type.or_else(|| {
            right_type
                .as_ref()
                .filter(|ty| is_quantity(ty))
                .and_then(|_| infer_speculative(left, Some(&scalar), context, node_types))
        });
        let right_type = right_type.or_else(|| {
            left_type
                .as_ref()
                .filter(|ty| is_quantity(ty))
                .and_then(|_| infer_speculative(right, Some(&scalar), context, node_types))
        });
        if let (Some(left_type), Some(right_type)) = (left_type, right_type) {
            if let Some(result) = quantity_result(operator, &left_type, &right_type, span) {
                return result;
            }
        }
        node_types.truncate(checkpoint);
    }
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
    let right_type = if is_quantity(&left_type)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Remainder
        ) {
        // A projection can reveal quantity meaning even when neither the input
        // record nor the expected result is itself a quantity. Only a proven
        // quantity operand enables this fallback; integer trees stay single-pass.
        infer_speculative(right, None, context, node_types)
            .or_else(|| {
                infer_speculative(
                    right,
                    Some(&CheckedExpressionType::semantic(
                        conduit_core::SCALAR_INFO_ID,
                    )),
                    context,
                    node_types,
                )
            })
            .ok_or_else(|| {
                diagnostic(
                    right.span(),
                    "quantity arithmetic requires a typed quantity or exact Scalar operand",
                )
            })?
    } else {
        infer(right, Some(&left_type), context, node_types)?
    };
    if let Some(result) = quantity_result(operator, &left_type, &right_type, span) {
        return result;
    }
    require(
        right_type,
        &left_type,
        span,
        "binary operands must have one exact type",
    )?;
    if left_type
        .value_kind()
        .is_some_and(|kind| kind.as_str() == conduit_core::MATHEMATICAL_VALUE_ID)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
        )
    {
        return Ok(left_type);
    }
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

fn quantity_result(
    operator: BinaryOperator,
    left: &CheckedExpressionType,
    right: &CheckedExpressionType,
    span: Span,
) -> Option<Result<CheckedExpressionType, ExpressionTypeDiagnostic>> {
    let (left, right) = (left.value_kind()?, right.value_kind()?);
    crate::expression_quantity::result_kind(operator, left.as_str(), right.as_str()).map(|result| {
        result.map(CheckedExpressionType::Semantic).map_err(|_| {
            diagnostic(
                span,
                "quantity operator requires compatible linear quantities or exact Scalar scaling",
            )
        })
    })
}

fn is_quantity(ty: &CheckedExpressionType) -> bool {
    ty.value_kind()
        .is_some_and(|kind| crate::expression_quantity::is_quantity(kind.as_str()))
}
fn infer_speculative(
    syntax: &ExpressionSyntax,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    nodes: &mut Vec<CheckedExpressionNodeType>,
) -> Option<CheckedExpressionType> {
    let checkpoint = nodes.len();
    let result = infer(syntax, expected, context, nodes);
    if result.is_err() {
        nodes.truncate(checkpoint);
    }
    result.ok()
}

fn physical_literal(syntax: &ExpressionSyntax) -> bool {
    match syntax {
        ExpressionSyntax::Atomic(value) => {
            conduit_core::Quantity::parse_plot_literal(&value.text).is_ok()
        }
        ExpressionSyntax::Binary { left, right, .. } => {
            physical_literal(left) || physical_literal(right)
        }
        ExpressionSyntax::Unary { operand, .. } => physical_literal(operand),
        _ => false,
    }
}
