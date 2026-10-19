//! Explicit evaluator coordinate policy; no runtime representation fallback.
use super::{storage_bound, Refusal};
use crate::PortableExpressionNode;
use conduit_core::{QuantityValueProfile, StructuredInfoType};

pub(super) fn output_capacity(
    node: &PortableExpressionNode,
    input: &StructuredInfoType,
    maximum: Option<u32>,
    profile: QuantityValueProfile,
) -> Result<usize, Refusal> {
    if profile != QuantityValueProfile::Decimal
        && super::leaf_info_kind(&node.value_type)
            .is_ok_and(crate::expression_quantity::is_quantity)
    {
        return Ok(profile.maximum_encoded_bytes());
    }
    storage_bound::expression(node, input, maximum)
}

pub(super) fn validate_input(
    kind: &str,
    bytes: &[u8],
    profile: QuantityValueProfile,
) -> Result<(), Refusal> {
    if crate::expression_quantity::is_quantity(kind) {
        validate_quantity(kind, bytes, profile)
    } else {
        conduit_core::validate_primitive_info(kind, bytes).map_err(|_| Refusal::InvalidInput)
    }
}

// Do not reserve optional Unit/bignum scratch in the ordinary scalar frame.
#[inline(never)]
fn validate_quantity(
    kind: &str,
    bytes: &[u8],
    profile: QuantityValueProfile,
) -> Result<(), Refusal> {
    profile
        .decode(
            kind,
            bytes,
            &mut conduit_core::ExactNumericWork::new(
                conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK,
            ),
        )
        .map(|_| ())
        .map_err(|_| Refusal::InvalidInput)
}

/// Match the existing portable decoder's finite node/depth envelope and refuse
/// structured intermediates whose exact-coordinate storage is not admitted yet.
pub(super) fn admit_primitive_nodes(root: &PortableExpressionNode) -> Result<(), Refusal> {
    fn visit(
        node: &PortableExpressionNode,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<(), Refusal> {
        use crate::PortableExpressionOperation as Op;
        if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH || *remaining == 0 {
            return Err(Refusal::UnsupportedType(
                "exact Quantity primitive program bound".into(),
            ));
        }
        *remaining -= 1;
        if !matches!(
            node.value_type.shape(),
            conduit_core::StructuredInfoTypeShape::Leaf(_)
        ) {
            return Err(Refusal::UnsupportedType(
                "exact Quantity profile requires primitive intermediates".into(),
            ));
        }
        let mut child = |node: &PortableExpressionNode| visit(node, depth + 1, remaining);
        match &node.operation {
            Op::Input | Op::Literal(_) | Op::CanonicalLiteral(_) | Op::Constant(_) => Ok(()),
            Op::ValueContract { value: operand, .. }
            | Op::Unary { operand, .. }
            | Op::Projection { value: operand, .. } => child(operand),
            Op::Binary { left, right, .. } => {
                child(left)?;
                child(right)
            }
            Op::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                child(condition)?;
                child(when_true)?;
                child(when_false)
            }
            Op::SemanticCall { arguments, .. } => {
                if super::leaf_info_kind(&node.value_type)
                    .is_ok_and(crate::expression_quantity::is_quantity)
                    || arguments.iter().any(|arg| {
                        super::leaf_info_kind(&arg.value_type)
                            .is_ok_and(crate::expression_quantity::is_quantity)
                    })
                {
                    return Err(Refusal::UnsupportedType(
                        "exact Quantity semantic call requires codec support".into(),
                    ));
                }
                for argument in arguments {
                    child(argument)?;
                }
                Ok(())
            }
            _ => Err(Refusal::UnsupportedType(
                "exact Quantity profile requires primitive operations".into(),
            )),
        }
    }
    let mut remaining = conduit_core::MAXIMUM_STRUCTURED_INFO_NODES;
    visit(root, 1, &mut remaining)
}

impl super::PreparedPortableExpressionEvaluator {
    /// Actual prepared payload ceiling (allocator bookkeeping and process stack
    /// excluded), plus node/depth/work bounds. None means this concrete tree's
    /// storage has not been reviewed for the installed exact profile.
    pub fn exact_quantity_profile_bounds(&self) -> Option<(usize, usize, usize, u64)> {
        use super::{PreparedInput, PreparedOperation as Op, PreparedRoot};
        if self.input.quantity_profile() != QuantityValueProfile::Exact {
            return None;
        }
        let PreparedRoot::Primitive {
            node,
            nominal_type: None,
        } = &self.root
        else {
            return None;
        };
        fn visit(
            node: &super::PreparedNode,
            depth: usize,
            nodes: &mut usize,
            maximum_depth: &mut usize,
        ) -> Option<usize> {
            *nodes = nodes.checked_add(1)?;
            *maximum_depth = (*maximum_depth).max(depth);
            let mut bytes = core::mem::size_of::<super::PreparedNode>()
                .checked_add(node.info_kind.capacity())?;
            let mut child =
                |node: &super::PreparedNode| visit(node, depth + 1, nodes, maximum_depth);
            let extra = match &node.operation {
                Op::Input => 0,
                Op::Literal(value) => value.capacity(),
                Op::Unary { operand, .. } | Op::Widen(operand) | Op::TextMaterial(operand) => {
                    child(operand)?
                }
                Op::Binary { left, right, .. } => child(left)?.checked_add(child(right)?)?,
                Op::QuantityBinary {
                    left,
                    right,
                    output,
                    ..
                } => output
                    .capacity()
                    .checked_add(child(left)?)?
                    .checked_add(child(right)?)?,
                Op::MathematicalBinary {
                    left,
                    right,
                    writer,
                    output,
                    ..
                } => output
                    .capacity()
                    .checked_add(writer.retained_payload_bytes())?
                    .checked_add(child(left)?)?
                    .checked_add(child(right)?)?,
                Op::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => child(condition)?
                    .checked_add(child(when_true)?)?
                    .checked_add(child(when_false)?)?,
                _ => return None,
            };
            bytes = bytes.checked_add(extra)?;
            Some(bytes)
        }
        let mut nodes = 0;
        let mut depth = 0;
        let mut payload = visit(node, 1, &mut nodes, &mut depth)?;
        let PreparedInput::Primitive {
            info_kind,
            nominal_type: None,
            ..
        } = &self.input
        else {
            return None;
        };
        payload = payload
            .checked_add(info_kind.capacity())?
            .checked_add(self.output.capacity())?
            .checked_add(core::mem::size_of::<Self>())?;
        // Every Quantity operation/comparison has its own 3x validation-work
        // meter. Count all nodes conservatively, including input/output checks.
        let work = (nodes as u64)
            .checked_mul(8 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK)?
            .checked_add(conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK)?;
        Some((nodes, depth, payload, work))
    }
}

// The first physical symbolic expression profile has deliberately finite support.
pub(super) fn admit_mathematical_angles(root: &PortableExpressionNode) -> Result<(), Refusal> {
    fn visit(node: &PortableExpressionNode, depth: usize, left: &mut usize) -> Result<(), Refusal> {
        use crate::PortableExpressionOperation as O;
        if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH || *left == 0 {
            return Err(Refusal::UnsupportedType(
                "mathematical Angle program bound".into(),
            ));
        }
        *left -= 1;
        if !matches!(
            node.value_type.shape(),
            conduit_core::StructuredInfoTypeShape::Leaf(_)
        ) {
            return Err(Refusal::UnsupportedType(
                "mathematical Angle primitive intermediates".into(),
            ));
        }
        let mut child = |n: &PortableExpressionNode| visit(n, depth + 1, left);
        match &node.operation {
            O::Input | O::Literal(_) | O::CanonicalLiteral(_) | O::Constant(_) => Ok(()),
            O::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                child(condition)?;
                child(when_true)?;
                child(when_false)
            }
            O::SemanticCall { kind, arguments }
                if kind == crate::expression_angle_reference::ANGLE_CALL
                    || crate::expression_trigonometry::function(kind).is_some() =>
            {
                for arg in arguments {
                    child(arg)?;
                }
                Ok(())
            }
            _ => Err(Refusal::UnsupportedType(
                "mathematical Angle operation/refinement requires reviewed profile support".into(),
            )),
        }
    }
    let mut left = conduit_core::MAXIMUM_STRUCTURED_INFO_NODES;
    visit(root, 1, &mut left)
}
