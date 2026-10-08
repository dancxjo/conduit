//! Conservative requested heap accounting; shared bytes are counted per owner.
use super::{
    PreparedInput, PreparedNode, PreparedOperation, PreparedPortableExpressionEvaluator,
    PreparedRoot,
};
use core::mem::size_of;

pub(super) fn boxed<T>(value: &T, heap: usize) -> usize {
    let _ = value;
    size_of::<T>().saturating_add(heap)
}
pub(super) fn shared(bytes: &super::SharedBytes) -> usize {
    // Arc/Rc slice layout includes tail padding to the control-block alignment.
    bytes
        .len()
        .saturating_add(2 * size_of::<usize>())
        .saturating_add(core::mem::align_of::<usize>() - 1)
}
impl PreparedPortableExpressionEvaluator {
    /// Conservative requested heap bytes retained by this evaluator. Shared
    /// schema allocations are overcounted for every owner; allocator bookkeeping
    /// and the evaluator value itself are excluded. Includes all spare capacity.
    pub fn owned_heap_bytes(&self) -> usize {
        let input = match &self.input {
            PreparedInput::Primitive { nominal_type, .. } => {
                nominal_type.as_ref().map_or(0, shared)
            }
            PreparedInput::Structured(bytes) => shared(bytes),
        };
        let root = match &self.root {
            PreparedRoot::Primitive { node, nominal_type } => node
                .owned_heap_bytes()
                .saturating_add(nominal_type.as_ref().map_or(0, |bytes| bytes.capacity())),
            PreparedRoot::Structured(value) => value.owned_heap_bytes(),
        };
        input
            .saturating_add(root)
            .saturating_add(self.output.capacity())
    }
}
impl PreparedNode {
    pub(super) fn owned_heap_bytes(&self) -> usize {
        match &self.operation {
            PreparedOperation::Input => 0,
            PreparedOperation::Literal(bytes) => bytes.capacity(),
            PreparedOperation::Unary { operand, .. }
            | PreparedOperation::Widen(operand)
            | PreparedOperation::TextMaterial(operand) => {
                boxed(operand.as_ref(), operand.owned_heap_bytes())
            }
            PreparedOperation::Binary { left, right, .. } => {
                boxed(left.as_ref(), left.owned_heap_bytes())
                    .saturating_add(boxed(right.as_ref(), right.owned_heap_bytes()))
            }
            PreparedOperation::Conditional {
                condition,
                when_true,
                when_false,
            } => boxed(condition.as_ref(), condition.owned_heap_bytes())
                .saturating_add(boxed(when_true.as_ref(), when_true.owned_heap_bytes()))
                .saturating_add(boxed(when_false.as_ref(), when_false.owned_heap_bytes())),
            PreparedOperation::Projection(value) => value.owned_heap_bytes(),
            PreparedOperation::Equality(value) => value.owned_heap_bytes(),
            PreparedOperation::SequenceSelection(value) => value.owned_heap_bytes(),
            PreparedOperation::Inspection(value) => value.owned_heap_bytes(),
            PreparedOperation::Bytes(value) => value.owned_heap_bytes(),
        }
    }
}
