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
    if profile == QuantityValueProfile::Exact
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
            Op::Unary { operand, .. } | Op::Projection { value: operand, .. } => child(operand),
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
