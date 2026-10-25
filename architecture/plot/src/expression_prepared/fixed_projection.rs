//! Checked input locations for fixed-width constructor children. The enclosing
//! evaluator must validate the entire canonical input and exact Type first.
use crate::PortableExpressionProjection as Projection;
use crate::{PortableExpressionNode as Node, PortableExpressionOperation as Op};
use conduit_core::{StructuredInfoType as Type, StructuredInfoTypeShape as Shape};

const MAX_DEPTH: u8 = 32;

/// A whole value node, including its canonical framing, but not its Type.
/// Unsupported/variable geometry retains ordinary member selection.
pub(super) fn prepare(input: &Type, node: &Node, prefix_bytes: usize) -> Option<(u32, u32)> {
    if !matches!(node.operation, Op::Projection { .. }) {
        return None;
    }
    let (start, selected) = locate(input, node, prefix_bytes, 0)?;
    let length = node_bytes(selected, 0)?;
    if start.checked_add(length)? > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return None;
    }
    Some((u32::try_from(start).ok()?, u32::try_from(length).ok()?))
}

fn locate<'a>(
    input: &'a Type,
    node: &'a Node,
    prefix_bytes: usize,
    depth: u8,
) -> Option<(usize, &'a Type)> {
    if depth == MAX_DEPTH {
        return None;
    }
    let (start, selected) = match &node.operation {
        Op::Input if node.value_type == *input => (prefix_bytes, input),
        Op::Projection { value, member } => {
            let (start, source) = locate(input, value, prefix_bytes, depth + 1)?;
            match (source.shape(), member) {
                (Shape::Record { fields, .. }, Projection::Field(name)) => {
                    let mut start = start.checked_add(5)?;
                    let mut selected = None;
                    for field in fields {
                        start = start.checked_add(4 + field.name().len())?;
                        if field.name() == name {
                            selected = Some((start, field.value_type()));
                            break;
                        }
                        start = start.checked_add(node_bytes(field.value_type(), 0)?)?;
                    }
                    selected?
                }
                (Shape::Record { fields, .. }, Projection::TupleIndex(index)) => {
                    let name = alloc::format!("item-{index:05}");
                    let mut start = start.checked_add(5)?;
                    let mut selected = None;
                    for field in fields {
                        start = start.checked_add(4 + field.name().len())?;
                        if field.name() == name {
                            selected = Some((start, field.value_type()));
                            break;
                        }
                        start = start.checked_add(node_bytes(field.value_type(), 0)?)?;
                    }
                    selected?
                }
                (Shape::Collection { element, length }, Projection::TupleIndex(index))
                    if *index < length =>
                {
                    let preceding = node_bytes(element, 0)?.checked_mul(usize::from(*index))?;
                    (start.checked_add(5)?.checked_add(preceding)?, element)
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    (selected == &node.value_type).then_some((start, selected))
}

fn node_bytes(ty: &Type, depth: u8) -> Option<usize> {
    if depth == MAX_DEPTH {
        return None;
    }
    match ty.shape() {
        Shape::Leaf(kind) => {
            let kind = conduit_core::primitive_info_kind(kind.as_str())?;
            let length = if kind == conduit_core::PrimitiveInfoKind::Count {
                conduit_core::COUNT_ENCODED_LEN
            } else {
                conduit_core::fixed_integer_bytes(kind)
            };
            if length != 0 {
                5usize.checked_add(length)
            } else if kind == conduit_core::PrimitiveInfoKind::Bool {
                Some(6)
            } else if kind == conduit_core::PrimitiveInfoKind::Empty {
                Some(5)
            } else {
                None
            }
        }
        Shape::Nominal { representation, .. } => node_bytes(representation, depth + 1),
        Shape::Collection { element, length } => node_bytes(element, depth + 1)?
            .checked_mul(usize::from(length))?
            .checked_add(5),
        Shape::Record { fields, .. } => fields.iter().try_fold(5usize, |bytes, field| {
            bytes
                .checked_add(4 + field.name().len())?
                .checked_add(node_bytes(field.value_type(), depth + 1)?)
        }),
        Shape::Variant { .. } | Shape::Sequence { .. } => None,
    }
}
