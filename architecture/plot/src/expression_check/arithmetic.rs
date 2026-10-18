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
    let quantity_arithmetic = matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Remainder
    );
    let checkpoint = node_types.len();
    let scalar = CheckedExpressionType::semantic(conduit_core::SCALAR_INFO_ID);
    // Infer each operand once on the successful path. A surrounding Quantity
    // input/result says nothing about the arithmetic in this subtree.
    let (left_type, inferred_right) = match infer(
        left,
        operand_context(left, expected, context),
        context,
        node_types,
    ) {
        Ok(value_type) => (value_type, None),
        Err(left_error) => {
            node_types.truncate(checkpoint);
            // A physical literal can establish its own type when the result's
            // context (for example Scalar for a ratio) is not its operand type.
            let natural_left = if quantity_arithmetic && expected.is_some() {
                infer_speculative(left, None, context, node_types)
            } else {
                None
            };
            if let Some(value_type) = natural_left {
                (value_type, None)
            } else {
                let right_type = infer_speculative(
                    right,
                    operand_context(right, expected, context),
                    context,
                    node_types,
                )
                .or_else(|| {
                    if quantity_arithmetic && expected.is_some() {
                        infer_speculative(right, None, context, node_types)
                    } else {
                        None
                    }
                })
                .ok_or(left_error.clone())?;
                let left_expected =
                    if operator == BinaryOperator::Multiply && is_quantity(&right_type) {
                        &scalar
                    } else {
                        &right_type
                    };
                let left_type = infer(
                    left,
                    operand_context(left, Some(left_expected), context),
                    context,
                    node_types,
                )
                .map_err(|_| left_error)?;
                // Reuse the successful right inference, rather than replaying
                // an arbitrarily large right subtree after contextual typing.
                (left_type, Some(right_type))
            }
        }
    };
    let right_type = if let Some(right_type) = inferred_right {
        right_type
    } else if is_quantity(&left_type) && quantity_arithmetic {
        let right_expected =
            if matches!(operator, BinaryOperator::Multiply | BinaryOperator::Divide) {
                &scalar
            } else {
                &left_type
            };
        // Scaling first gives a constant scalar subtree its required context.
        // A genuine mismatch may instead reveal a typed quantity for a ratio.
        infer_speculative(
            right,
            operand_context(right, Some(right_expected), context),
            context,
            node_types,
        )
        .or_else(|| infer_speculative(right, None, context, node_types))
        .ok_or_else(|| {
            diagnostic(
                right.span(),
                "quantity arithmetic requires a typed quantity or exact Scalar operand",
            )
        })?
    } else {
        let checkpoint = node_types.len();
        match infer(right, Some(&left_type), context, node_types) {
            Ok(value_type) => value_type,
            Err(error) if operator == BinaryOperator::Multiply && left_type == scalar => {
                node_types.truncate(checkpoint);
                infer(right, None, context, node_types).map_err(|_| error)?
            }
            Err(error) => return Err(error),
        }
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

// Context may construct a generic Quantity literal at a value boundary, but an
// operator must retain the resolved physical operand Type to choose its law.
fn operand_context<'a>(
    syntax: &ExpressionSyntax,
    expected: Option<&'a CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
) -> Option<&'a CheckedExpressionType> {
    if expected.is_some_and(|ty| {
        ty.value_kind()
            .is_some_and(|kind| kind.as_str() == conduit_core::QUANTITY_INFO_ID)
    }) {
        if let ExpressionSyntax::Atomic(value) = syntax {
            if context
                .literal_types
                .get(&value.text)
                .is_some_and(is_quantity)
            {
                return None;
            }
        }
    }
    expected
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
