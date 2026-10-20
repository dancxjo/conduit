//! Bind checked physical nodes to a fixed postorder program with one shared input.
use super::*;
use crate::{BinaryOperator as B, UnaryOperator as U};

pub(super) fn lower(
    root: &PortableExpressionNode,
    input: &StructuredInfoType,
    coordinate: Quantity,
    maximum_steps: u8,
    work: &mut ExactNumericWork,
) -> Result<Vec<Binary64Operation>, R> {
    let maximum = usize::from(maximum_steps).min(BINARY64_ALGORITHM_MAX_STEPS);
    if maximum == 0 {
        return Err(Binary64AlgorithmRefusal::StepLimit.into());
    }
    let mut operations = Vec::with_capacity(maximum);
    operations.push(Binary64Operation::Constant(Box::new(ExactRational::ZERO)));
    let result = visit(root, input, coordinate, &mut operations, maximum, 0, work)?;
    if result.unit != Some(coordinate.unit()) || !quantity(kind(root)?) {
        return Err(R::Physical);
    }
    Ok(operations)
}

struct Node {
    step: u8,
    unit: Option<Unit>,
}
fn kind(node: &PortableExpressionNode) -> Result<&str, R> {
    let StructuredInfoTypeShape::Leaf(kind) = node.value_type.shape() else {
        return Err(R::UnsupportedOperation);
    };
    Ok(kind.as_str())
}
fn quantity(kind: &str) -> bool {
    crate::expression_quantity::is_quantity(kind)
}
fn push(
    operations: &mut Vec<Binary64Operation>,
    maximum: usize,
    operation: Binary64Operation,
) -> Result<u8, R> {
    if operations.len() >= maximum {
        return Err(Binary64AlgorithmRefusal::StepLimit.into());
    }
    let step = operations.len() as u8;
    operations.push(operation);
    Ok(step)
}

fn visit(
    node: &PortableExpressionNode,
    input: &StructuredInfoType,
    coordinate: Quantity,
    operations: &mut Vec<Binary64Operation>,
    maximum: usize,
    depth: usize,
    work: &mut ExactNumericWork,
) -> Result<Node, R> {
    if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
        return Err(R::Program);
    }
    let node_kind = kind(node)?;
    let result = match &node.operation {
        Op::Input if &node.value_type == input && quantity(node_kind) => Node {
            step: 0,
            unit: Some(coordinate.unit()),
        },
        Op::CanonicalLiteral(bytes) => {
            constant(node_kind, bytes, coordinate, operations, maximum, work)?
        }
        Op::Literal(literal) => {
            let bytes =
                crate::expression_evaluate::literal_primitive_bytes(&node.value_type, literal)
                    .map_err(|_| R::Program)?;
            constant(node_kind, &bytes, coordinate, operations, maximum, work)?
        }
        Op::Constant(value) if value.value_type() == &node.value_type => {
            let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                return Err(R::UnsupportedOperation);
            };
            constant(node_kind, bytes, coordinate, operations, maximum, work)?
        }
        Op::Unary {
            operator: U::Negate,
            operand,
        } => {
            let value = visit(
                operand,
                input,
                coordinate,
                operations,
                maximum,
                depth + 1,
                work,
            )?;
            if value.unit.is_none() || kind(operand)? != node_kind {
                return Err(R::UnsupportedOperation);
            }
            Node {
                step: push(
                    operations,
                    maximum,
                    Binary64Operation::Negate { input: value.step },
                )?,
                unit: value.unit,
            }
        }
        Op::Binary {
            operator,
            left,
            right,
            proven: false,
        } => {
            let left_kind = kind(left)?;
            let right_kind = kind(right)?;
            let expected =
                crate::expression_quantity::result_kind(*operator, left_kind, right_kind)
                    .and_then(Result::ok)
                    .ok_or(R::UnsupportedOperation)?;
            if expected.as_str() != node_kind {
                return Err(R::Program);
            }
            let left = visit(
                left,
                input,
                coordinate,
                operations,
                maximum,
                depth + 1,
                work,
            )?;
            let right = visit(
                right,
                input,
                coordinate,
                operations,
                maximum,
                depth + 1,
                work,
            )?;
            // Fixed-point Scalar operations are not reinterpreted as real arithmetic.
            // Only already-denoted Scalar factors enter this physical profile.
            let result_unit = match (*operator, left.unit, right.unit) {
                (B::Add | B::Subtract, Some(a), Some(b)) if a == b => Some(a),
                (B::Multiply, Some(a), None) | (B::Multiply, None, Some(a)) => Some(a),
                (B::Divide, Some(a), None) => Some(a),
                _ => return Err(R::UnsupportedOperation),
            };
            let operation = match operator {
                B::Add => Binary64Operation::Add {
                    left: left.step,
                    right: right.step,
                },
                B::Subtract => Binary64Operation::Subtract {
                    left: left.step,
                    right: right.step,
                },
                B::Multiply => Binary64Operation::Multiply {
                    left: left.step,
                    right: right.step,
                },
                B::Divide => Binary64Operation::Divide {
                    left: left.step,
                    right: right.step,
                },
                _ => return Err(R::UnsupportedOperation),
            };
            Node {
                step: push(operations, maximum, operation)?,
                unit: result_unit,
            }
        }
        _ => return Err(R::UnsupportedOperation),
    };
    Ok(result)
}

fn constant(
    kind: &str,
    bytes: &[u8],
    coordinate: Quantity,
    operations: &mut Vec<Binary64Operation>,
    maximum: usize,
    work: &mut ExactNumericWork,
) -> Result<Node, R> {
    let (value, physical) = if quantity(kind) {
        let value = QuantityValueProfile::Mathematical
            .decode(kind, bytes, work)
            .map_err(|error| match error {
                QuantityValueContractRefusal::Encoding(error) => quantity_error(error),
                _ => R::Physical,
            })?;
        if value.role() != coordinate.role() {
            return Err(R::UnsupportedRole);
        }
        require_coordinate(value.unit(), coordinate.unit())?;
        let value = value.rational(work).map_err(quantity_error)?;
        (value.coordinate(), Some(coordinate.unit()))
    } else if kind == SCALAR_INFO_ID {
        let scalar = Scalar::decode(bytes).map_err(|_| R::Program)?;
        (
            ExactRational::ratio(
                i128::from(scalar.raw_microunits()),
                i128::from(Scalar::SCALE),
                work,
            )?,
            None,
        )
    } else {
        return Err(R::UnsupportedOperation);
    };
    Ok(Node {
        step: push(
            operations,
            maximum,
            Binary64Operation::Constant(Box::new(value)),
        )?,
        unit: physical,
    })
}

fn quantity_error(error: QuantityValueRefusal) -> R {
    match error {
        QuantityValueRefusal::Rational(RationalQuantityRefusal::Numeric(error)) => R::Work(error),
        QuantityValueRefusal::ExtentExceeded => R::Storage,
        _ => R::UnsupportedDenotation,
    }
}
