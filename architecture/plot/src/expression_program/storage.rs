//! Requested heap accounting of decoded expression metadata before preparation.
use super::{
    PortableExpressionNode, PortableExpressionOperation as Operation, PortableExpressionProgram,
    PortableExpressionProjection,
};
use core::mem::size_of;
fn boxed(node: &PortableExpressionNode) -> usize {
    size_of::<PortableExpressionNode>().saturating_add(node.owned_heap_bytes())
}
impl PortableExpressionProgram {
    /// Requested heap bytes owned by this decoded program, excluding its root
    /// stack value and allocator bookkeeping. Includes spare vector capacities.
    pub fn owned_heap_bytes(&self) -> usize {
        self.input_type
            .owned_heap_bytes()
            .saturating_add(self.output_type.owned_heap_bytes())
            .saturating_add(self.root.owned_heap_bytes())
    }
}
impl PortableExpressionNode {
    fn owned_heap_bytes(&self) -> usize {
        let operation = match &self.operation {
            Operation::Input => 0,
            Operation::Literal(text) => text.capacity(),
            Operation::Projection { value, member } => boxed(value).saturating_add(match member {
                PortableExpressionProjection::Field(name) => name.capacity(),
                PortableExpressionProjection::TupleIndex(_) => 0,
            }),
            Operation::Unary { operand, .. } => boxed(operand),
            Operation::Binary { left, right, .. } => boxed(left).saturating_add(boxed(right)),
            Operation::Conditional {
                condition,
                when_true,
                when_false,
            } => boxed(condition)
                .saturating_add(boxed(when_true))
                .saturating_add(boxed(when_false)),
            Operation::Tuple(values) | Operation::Collection(values) => values.iter().fold(
                values
                    .capacity()
                    .saturating_mul(size_of::<PortableExpressionNode>()),
                |total, value| total.saturating_add(value.owned_heap_bytes()),
            ),
            Operation::Record(fields) => fields.iter().fold(
                fields
                    .capacity()
                    .saturating_mul(size_of::<(alloc::string::String, PortableExpressionNode)>()),
                |total, (name, value)| {
                    total
                        .saturating_add(name.capacity())
                        .saturating_add(value.owned_heap_bytes())
                },
            ),
            Operation::Variant { tag, payload } => tag.capacity().saturating_add(boxed(payload)),
            Operation::SemanticCall { kind, arguments } => arguments.iter().fold(
                kind.capacity().saturating_add(
                    arguments
                        .capacity()
                        .saturating_mul(size_of::<PortableExpressionNode>()),
                ),
                |total, value| total.saturating_add(value.owned_heap_bytes()),
            ),
        };
        self.value_type.owned_heap_bytes().saturating_add(operation)
    }
}
