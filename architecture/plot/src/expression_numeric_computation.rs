//! Lower checked mathematical operations to an ordered, source-bound graph.
use crate::{BinaryOperator, CheckedExpression, ExpressionSyntax, Span, UnaryOperator};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use conduit_core::{
    MathValue, NumericComputation, NumericComputationNode, NumericComputationOperation as Op,
    NumericComputationRefusal, NumericOperation, Quantity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericExpressionRefusal {
    MissingCheckedType,
    UnsupportedOperation,
    MissingLiteralEvidence,
    InvalidNumber(conduit_core::NumericRefusal),
    InvalidQuantity,
    Computation(NumericComputationRefusal),
    Evaluation(conduit_core::NumericEvaluationRefusal),
}

/// Preserves authored operation order even when the closed denotation simplifies.
/// Explicit machine-integer and IEEE programs keep their existing realization path.
pub fn checked_numeric_computation(
    expression: &CheckedExpression,
) -> Result<Option<NumericComputation>, NumericExpressionRefusal> {
    let root_kind = expression.value_type.value_kind().map(|kind| kind.as_str());
    if !root_kind.is_some_and(|kind| is_scalar_kind(kind) || kind == conduit_core::F64_INFO_ID)
        && !contains_mathematical_call(&expression.syntax)
        && !expression.node_types.iter().any(|node| {
            node.value_type.value_kind().is_some_and(|kind| {
                is_scalar_kind(kind.as_str()) || kind.as_str() == conduit_core::F64_INFO_ID
            })
        })
    {
        return Ok(None);
    }
    let mut nodes = Vec::new();
    let root = lower(&expression.syntax, expression, &mut nodes)?;
    let graph =
        NumericComputation::new(nodes, root).map_err(NumericExpressionRefusal::Computation)?;
    if !graph.nodes().iter().any(|node| {
        matches!(node.operation, Op::Parameter(_))
            || node.semantic_kind == conduit_core::F64_INFO_ID
    }) {
        graph
            .evaluate_exact(&[])
            .map_err(NumericExpressionRefusal::Evaluation)?;
    }
    Ok(Some(graph))
}

fn kind(expression: &CheckedExpression, span: Span) -> Result<String, NumericExpressionRefusal> {
    expression
        .node_types
        .iter()
        .find(|node| node.span.start == span.start && node.span.end == span.end)
        .and_then(|node| node.value_type.value_kind())
        .map(|kind| kind.as_str().into())
        .ok_or(NumericExpressionRefusal::MissingCheckedType)
}
fn lower(
    syntax: &ExpressionSyntax,
    expression: &CheckedExpression,
    nodes: &mut Vec<NumericComputationNode>,
) -> Result<u16, NumericExpressionRefusal> {
    let semantic_kind = kind(expression, syntax.span())?;
    let mut unit = None;
    let mut reference_unit = None;
    let operation = match syntax {
        ExpressionSyntax::Input(_) => Op::Parameter(0),
        ExpressionSyntax::Atomic(value) if semantic_kind == conduit_core::F64_INFO_ID => {
            let value = MathValue::parse_decimal(&value.text).map_err(NumericExpressionRefusal::InvalidNumber)?;
            conduit_core::ieee_rounded_literal(&value).map_err(NumericExpressionRefusal::Computation)?

        }
        ExpressionSyntax::Atomic(value) => {
            let number = if is_scalar_kind(&semantic_kind) {
                if matches!(value.text.as_str(), "pi" | "π") {
                    MathValue::pi()
                } else {
                    MathValue::parse_decimal(&value.text)
                        .map_err(NumericExpressionRefusal::InvalidNumber)?
                }
            } else if conduit_core::parse_quantity_semantic_role(&semantic_kind).is_some() {
                // The immutable checker supplied this exact capsule: no catalogue reparse.
                let bytes = expression
                    .canonical_literals
                    .get(&(value.span.start, value.span.end))
                    .ok_or(NumericExpressionRefusal::MissingLiteralEvidence)?;
                let quantity = Quantity::decode(bytes)
                    .map_err(|_| NumericExpressionRefusal::InvalidQuantity)?;
                conduit_core::validate_primitive_info(&semantic_kind, bytes)
                    .map_err(|_| NumericExpressionRefusal::InvalidQuantity)?;
                unit = Some(Box::new(quantity.unit().to_owned()));
                quantity
                    .coordinate()
                    .to_owned()
                    .map_err(NumericExpressionRefusal::InvalidNumber)?
            } else {
                return Err(NumericExpressionRefusal::UnsupportedOperation);
            };
            Op::Literal(
                number
                    .canonical_bytes()
                    .map_err(NumericExpressionRefusal::InvalidNumber)?,
            )
        }
        ExpressionSyntax::Unary {
            operator: UnaryOperator::Negate,
            operand,
            ..
        } => {
            let operand = lower(operand, expression, nodes)?;
            Op::Apply {
                operation: NumericOperation::Negate,
                operands: vec![operand],
            }
        }
        ExpressionSyntax::Binary {
            operator,
            left,
            right,
            ..
        } => {
            let operation = binary(*operator)?;
            let left = lower(left, expression, nodes)?;
            let right = lower(right, expression, nodes)?;
            Op::Apply {
                operation,
                operands: vec![left, right],
            }
        }
        ExpressionSyntax::SemanticCall { arguments, span, .. }
            if expression.quantity_construction_units.contains_key(&(span.start, span.end)) => {
            unit = Some(Box::new(expression.quantity_construction_units[&(span.start, span.end)].clone()));
            let coordinate = lower(&arguments[0], expression, nodes)?;
            Op::Apply { operation: NumericOperation::QuantityConstruction, operands: vec![coordinate] }
        }
        ExpressionSyntax::SemanticCall { kind, arguments, .. }
            if kind.text == "F64" => {
            let coordinate = arguments.first().ok_or(NumericExpressionRefusal::MissingLiteralEvidence)?;
            let (value, _) = crate::expression_scalar::checked_scalar_expression(coordinate)
                .map_err(|_| NumericExpressionRefusal::MissingLiteralEvidence)?;
            conduit_core::ieee_rounded_literal(&value)
                .map_err(NumericExpressionRefusal::Computation)?
        }
        ExpressionSyntax::SemanticCall { kind, arguments, .. }
            if kind.text == "ieee/binary64-bits" => {
            let Some(ExpressionSyntax::Atomic(value)) = arguments.first() else {
                return Err(NumericExpressionRefusal::MissingLiteralEvidence);
            };
            Op::IeeeLiteral { bits: value.text.parse::<u64>()
                .map_err(|_| NumericExpressionRefusal::MissingLiteralEvidence)? }
        }
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => {
            let operation = call(&kind.text)?;
            if matches!(operation, NumericOperation::CyclicToAngularFrequency | NumericOperation::AngularToCyclicFrequency) {
                let references = expression.numeric_frequency_references.as_ref().ok_or(NumericExpressionRefusal::MissingLiteralEvidence)?;
                let (source, target) = if operation == NumericOperation::CyclicToAngularFrequency { (0, 1) } else { (1, 0) };
                reference_unit = Some(Box::new(references[source].clone()));
                unit = Some(Box::new(references[target].clone()));
            }

            if matches!(
                operation,
                NumericOperation::Sin
                    | NumericOperation::Cos
                    | NumericOperation::Tan
                    | NumericOperation::Asin
                    | NumericOperation::Acos
                    | NumericOperation::Atan
                    | NumericOperation::Atan2
            ) {
                reference_unit = Some(Box::new(
                    expression
                        .numeric_angle_reference
                        .clone()
                        .ok_or(NumericExpressionRefusal::UnsupportedOperation)?,
                ));
                if matches!(
                    operation,
                    NumericOperation::Asin
                        | NumericOperation::Acos
                        | NumericOperation::Atan
                        | NumericOperation::Atan2
                ) {
                    unit = reference_unit.clone();
                }
            }
            let operands = arguments
                .iter()
                .map(|argument| lower(argument, expression, nodes))
                .collect::<Result<Vec<_>, _>>()?;
            Op::Apply {
                operation,
                operands,
            }
        }
        _ => return Err(NumericExpressionRefusal::UnsupportedOperation),
    };
    if nodes.len() >= conduit_core::MAX_NUMERIC_COMPUTATION_NODES {
        return Err(NumericExpressionRefusal::Computation(
            NumericComputationRefusal::NodeBudget,
        ));
    }
    let index = nodes.len() as u16;
    nodes.push(NumericComputationNode {
        semantic_kind,
        unit,
        reference_unit,
        operation,
    });
    Ok(index)
}
fn binary(value: BinaryOperator) -> Result<NumericOperation, NumericExpressionRefusal> {
    Ok(match value {
        BinaryOperator::Add => NumericOperation::Add,
        BinaryOperator::Subtract => NumericOperation::Subtract,
        BinaryOperator::Multiply => NumericOperation::Multiply,
        BinaryOperator::Divide => NumericOperation::Divide,
        BinaryOperator::Equal => NumericOperation::Equal,
        BinaryOperator::NotEqual => NumericOperation::NotEqual,
        BinaryOperator::Less => NumericOperation::Less,
        BinaryOperator::LessOrEqual => NumericOperation::LessOrEqual,
        BinaryOperator::Greater => NumericOperation::Greater,
        BinaryOperator::GreaterOrEqual => NumericOperation::GreaterOrEqual,
        _ => return Err(NumericExpressionRefusal::UnsupportedOperation),
    })
}
fn call(value: &str) -> Result<NumericOperation, NumericExpressionRefusal> {
    Ok(match value {
        "math/pow" => NumericOperation::Power,
        "math/root" => NumericOperation::Root,
        "math/sqrt" => NumericOperation::SquareRoot,
        "math/sin" => NumericOperation::Sin,
        "math/cos" => NumericOperation::Cos,
        "math/tan" => NumericOperation::Tan,
        "math/asin" => NumericOperation::Asin,
        "math/acos" => NumericOperation::Acos,
        "math/atan" => NumericOperation::Atan,
        "math/atan2" => NumericOperation::Atan2,
        "math/cyclic-to-angular-frequency" => NumericOperation::CyclicToAngularFrequency,
        "math/angular-to-cyclic-frequency" => NumericOperation::AngularToCyclicFrequency,
        _ => return Err(NumericExpressionRefusal::UnsupportedOperation),
    })
}

pub(crate) fn contains_mathematical_call(value: &ExpressionSyntax) -> bool {
    match value {
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => call(&kind.text).is_ok() || arguments.iter().any(contains_mathematical_call),
        ExpressionSyntax::Unary { operand, .. } => contains_mathematical_call(operand),
        ExpressionSyntax::Binary { left, right, .. } => {
            contains_mathematical_call(left) || contains_mathematical_call(right)
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => [condition, when_true, when_false]
            .iter()
            .any(|value| contains_mathematical_call(value)),
        _ => false,
    }
}

fn is_scalar_kind(kind: &str) -> bool {
    conduit_core::primitive_info_kind(kind)
        == Some(conduit_core::PrimitiveInfoKind::MathematicalNumber)
}
