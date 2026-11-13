//! Cold selection of the existing fixed-scalar profile, never a larger grant.
use crate::expression_prepared::scalar_tape::{supported, MAXIMUM_INSTRUCTIONS};
use crate::{PortableExpressionNode as Node, PortableExpressionOperation as Op};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
pub(super) fn fits(node: &Node, input: &StructuredInfoType) -> bool {
    count(node, input, 0).is_some()
}
fn direct(node: &Node, input: &StructuredInfoType) -> bool {
    match &node.operation {
        Op::Input => &node.value_type == input,
        Op::Projection { value, .. } => direct(value, input),
        _ => false,
    }
}
fn count(node: &Node, input: &StructuredInfoType, depth: usize) -> Option<usize> {
    if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH {
        return None;
    }
    let StructuredInfoTypeShape::Leaf(kind) = node.value_type.shape() else {
        return None;
    };
    if !conduit_core::primitive_info_kind(kind.as_str()).is_some_and(supported) {
        return None;
    }
    let child = |n| count(n, input, depth + 1);
    let n = match &node.operation {
        Op::Input if &node.value_type == input => 1,
        Op::Literal(_) | Op::CanonicalLiteral(_) | Op::Constant(_) => 1,
        Op::Projection { .. } if direct(node, input) => 1,
        Op::Unary { operand, .. } => child(operand)?.checked_add(1)?,
        Op::Binary { left, right, .. } => {
            child(left)?.checked_add(child(right)?)?.checked_add(1)?
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => child(condition)?
            .checked_add(child(when_true)?)?
            .checked_add(child(when_false)?)?
            .checked_add(4)?,
        _ => return None,
    };
    (n <= MAXIMUM_INSTRUCTIONS).then_some(n)
}
