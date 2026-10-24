//! Conservative eligibility only; actual Source verification remains mandatory.
use super::*;
use conduit_core::StructuredInfoTypeShape as Shape;
impl PortableExpressionProgram {
    pub fn requires_native_law_custody(&self) -> bool {
        let mut remaining = 4096usize;
        requires_type(&self.input_type, 0, &mut remaining)
            || requires_type(&self.output_type, 0, &mut remaining)
            || requires_node(&self.root, 0, &mut remaining)
    }
}
fn charge(depth: usize, remaining: &mut usize) -> bool {
    if depth > 64 || *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    true
}
fn requires_type(ty: &StructuredInfoType, depth: usize, remaining: &mut usize) -> bool {
    if !charge(depth, remaining) {
        return true;
    }
    match ty.shape() {
        Shape::Leaf(kind) => {
            conduit_core::primitive_info_kind(kind.as_str()).is_none()
                && kind.as_str() != conduit_core::RESOURCE_REFERENCE_INFO_ID
        }
        Shape::Nominal { .. } => true,
        Shape::Record { fields, .. } => {
            !crate::rust_binding::exact_structural_constructor(ty)
                || fields
                    .iter()
                    .any(|field| requires_type(field.value_type(), depth + 1, remaining))
        }
        Shape::Variant { cases, .. } => {
            !crate::rust_binding::exact_structural_constructor(ty)
                || cases
                    .iter()
                    .any(|case| requires_type(case.payload_type(), depth + 1, remaining))
        }
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            requires_type(element, depth + 1, remaining)
        }
    }
}
fn requires_node(node: &PortableExpressionNode, depth: usize, remaining: &mut usize) -> bool {
    if !charge(depth, remaining) || requires_type(&node.value_type, depth, remaining) {
        return true;
    }
    use PortableExpressionOperation::*;
    match &node.operation {
        Input | Literal(_) | CanonicalLiteral(_) | Constant(_) => false,
        ValueContract { value, .. } | Projection { value, .. } => {
            requires_node(value, depth + 1, remaining)
        }
        Unary { operand, .. } => requires_node(operand, depth + 1, remaining),
        Binary { left, right, .. } => {
            requires_node(left, depth + 1, remaining) || requires_node(right, depth + 1, remaining)
        }
        Conditional {
            condition,
            when_true,
            when_false,
        } => {
            requires_node(condition, depth + 1, remaining)
                || requires_node(when_true, depth + 1, remaining)
                || requires_node(when_false, depth + 1, remaining)
        }
        Tuple(items)
        | Collection(items)
        | SemanticCall {
            arguments: items, ..
        } => items
            .iter()
            .any(|item| requires_node(item, depth + 1, remaining)),
        Record(fields) => fields
            .iter()
            .any(|(_, item)| requires_node(item, depth + 1, remaining)),
        Variant { payload, .. } => requires_node(payload, depth + 1, remaining),
    }
}
