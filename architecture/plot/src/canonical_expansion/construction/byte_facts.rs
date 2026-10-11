//! Branch-local proof of bounds on unchanged Bytes input projections.
//! This proves only direct length comparisons, never arithmetic or where laws.
use super::{Op, PortableExpressionNode, StructuredInfoTypeShape, refinement::input_path};
use crate::{BinaryOperator as Binary, UnaryOperator};
use alloc::{string::String, vec::Vec};
use conduit_core::{CheckedValueContract, ValueConstraint};

#[derive(Clone, Default)]
pub(super) struct ByteFacts(Vec<ByteBound>);
#[derive(Clone)]
struct ByteBound {
    path: String,
    minimum: u64,
    maximum: u64,
}

impl ByteFacts {
    pub(super) fn branch(&self, condition: &PortableExpressionNode, truth: bool) -> Self {
        let mut facts = self.clone();
        facts.observe(condition, truth);
        facts
    }

    pub(super) fn proves(
        &self,
        value: &PortableExpressionNode,
        remaining: &str,
        required: &CheckedValueContract,
    ) -> bool {
        if required.value_kind.as_str() != "value/bytes" {
            return false;
        }
        let Some(mut path) = input_path(value) else {
            return false;
        };
        path.push_str(remaining);
        self.0.iter().filter(|fact| fact.path == path).any(|fact| {
            fact.minimum <= fact.maximum
                && fact.maximum <= u64::from(required.maximum_bytes)
                && required
                    .constraints
                    .iter()
                    .all(|constraint| match constraint {
                        ValueConstraint::ByteLength { minimum, maximum } => {
                            fact.minimum >= u64::from(*minimum)
                                && fact.maximum <= u64::from(*maximum)
                        }
                        _ => false,
                    })
        })
    }

    fn observe(&mut self, node: &PortableExpressionNode, truth: bool) {
        match &node.operation {
            Op::Unary {
                operator: UnaryOperator::Not,
                operand,
            } => self.observe(operand, !truth),
            Op::Binary {
                operator: Binary::BooleanAnd,
                left,
                right,
                ..
            } if truth => {
                self.observe(left, true);
                self.observe(right, true);
            }
            Op::Binary {
                operator: Binary::BooleanOr,
                left,
                right,
                ..
            } if !truth => {
                self.observe(left, false);
                self.observe(right, false);
            }
            Op::Binary {
                operator,
                left,
                right,
                ..
            } => {
                if let Some((path, literal)) = comparison(left, right) {
                    self.comparison(path, *operator, literal, truth);
                }
            }
            _ => {}
        }
    }

    fn comparison(&mut self, path: String, operator: Binary, literal: u64, truth: bool) {
        let bound = match (operator, truth) {
            (Binary::LessOrEqual, true) | (Binary::Greater, false) => Some((0, literal)),
            (Binary::Less, true) | (Binary::GreaterOrEqual, false) => {
                literal.checked_sub(1).map(|max| (0, max))
            }
            (Binary::GreaterOrEqual, true) | (Binary::Less, false) => Some((literal, u64::MAX)),
            (Binary::Greater, true) | (Binary::LessOrEqual, false) => {
                literal.checked_add(1).map(|min| (min, u64::MAX))
            }
            (Binary::Equal, true) | (Binary::NotEqual, false) => Some((literal, literal)),
            _ => None,
        };
        let Some((minimum, maximum)) = bound else {
            return;
        };
        if let Some(fact) = self.0.iter_mut().find(|fact| fact.path == path) {
            fact.minimum = fact.minimum.max(minimum);
            fact.maximum = fact.maximum.min(maximum);
        } else {
            self.0.push(ByteBound {
                path,
                minimum,
                maximum,
            });
        }
    }
}

fn comparison(
    left: &PortableExpressionNode,
    right: &PortableExpressionNode,
) -> Option<(String, u64)> {
    let Op::SemanticCall { kind, arguments } = &left.operation else {
        return None;
    };
    if kind != "bytes/length" {
        return None;
    }
    let [bytes] = arguments.as_slice() else {
        return None;
    };
    if !matches!(bytes.value_type.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/bytes")
        || !matches!(right.value_type.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/u64")
    {
        return None;
    }
    let Op::Literal(literal) = &right.operation else {
        return None;
    };
    Some((input_path(bytes)?, literal.parse().ok()?))
}
