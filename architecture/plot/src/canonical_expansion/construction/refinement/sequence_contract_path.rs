//! Recover exact all-element Bytes/finite-F32 metadata through successful selection.
//! This path is never a branch-fact identity and cannot establish a narrower law.
use super::{
    input_path, Op, PortableExpressionNode, PortableExpressionProjection, String,
    StructuredInfoTypeShape,
};
use conduit_core::{CheckedValueContract, ValueConstraint};

pub(super) fn declared_path(
    node: &PortableExpressionNode,
    required: &CheckedValueContract,
) -> Option<String> {
    if !supported(required) {
        return None;
    }
    path(node)
}
pub(super) fn supported(required: &CheckedValueContract) -> bool {
    (required.value_kind.as_str() == "value/bytes"
        && required
            .constraints
            .iter()
            .all(|c| matches!(c, ValueConstraint::ByteLength { .. })))
        || (required.value_kind.as_str() == conduit_core::F32_INFO_ID
            && required.constraints.as_slice() == [ValueConstraint::FloatFinite])
}
fn representation(mut ty: &conduit_core::StructuredInfoType) -> &conduit_core::StructuredInfoType {
    while let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        ty = representation;
    }
    ty
}
fn path(node: &PortableExpressionNode) -> Option<String> {
    if let Some(path) = input_path(node) {
        return Some(path);
    }
    match &node.operation {
        Op::SemanticCall { kind, arguments } if kind == "sequence/at" => {
            let [source, index] = arguments.as_slice() else {
                return None;
            };
            let element = match representation(&source.value_type).shape() {
                StructuredInfoTypeShape::Collection { element, .. }
                | StructuredInfoTypeShape::Sequence { element, .. } => element,
                _ => return None,
            };
            if element != &node.value_type
                || index.value_type
                    != conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64"))
                        .ok()?
            {
                return None;
            }
            // Both evaluators refuse out-of-range selection. Any returned value
            // is the unchanged validated element; dynamic indices do not weaken
            // the declaration applying to every element of this exact container.
            let mut path = path(source)?;
            path.push_str("[]");
            Some(path)
        }
        Op::Projection {
            value,
            member: PortableExpressionProjection::Field(field),
        } => {
            let separator = match value.value_type.shape() {
                StructuredInfoTypeShape::Record { fields, .. }
                    if fields.iter().any(|member| member.name() == field) =>
                {
                    '.'
                }
                StructuredInfoTypeShape::Variant { cases, .. }
                    if cases.iter().any(|case| case.tag() == field) =>
                {
                    '|'
                }
                _ => return None,
            };
            let mut path = path(value)?;
            path.push(separator);
            path.push_str(field);
            Some(path)
        }
        // Tuple selection is deliberately excluded unless input_path already
        // established its exact identity. No generic shape-only forwarding.
        _ => None,
    }
}
