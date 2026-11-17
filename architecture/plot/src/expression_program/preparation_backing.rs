use super::*;
use crate::preparation_backing::{sum, Backing};
impl Backing for PortableExpressionProgram {
    fn backing(&self) -> Option<usize> {
        sum([
            self.input_type.backing(),
            self.output_type.backing(),
            self.root.backing(),
        ])
    }
}
impl Backing for PortableExpressionNode {
    fn backing(&self) -> Option<usize> {
        sum([
            self.value_type.backing(),
            match &self.operation {
                PortableExpressionOperation::Input => Some(0),
                PortableExpressionOperation::Literal(v) => v.backing(),
                PortableExpressionOperation::CanonicalLiteral(v) => Some(v.capacity()),
                PortableExpressionOperation::Constant(_) => None,
                PortableExpressionOperation::ValueContract {
                    input,
                    output,
                    value,
                } => sum([input.backing(), output.backing(), value.backing()]),
                PortableExpressionOperation::Projection { value, member } => sum([
                    value.backing(),
                    match member {
                        PortableExpressionProjection::Field(v) => v.backing(),
                        PortableExpressionProjection::TupleIndex(_) => Some(0),
                    },
                ]),
                PortableExpressionOperation::Unary { operand, .. } => operand.backing(),
                PortableExpressionOperation::Binary { left, right, .. } => {
                    sum([left.backing(), right.backing()])
                }
                PortableExpressionOperation::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => sum([
                    condition.backing(),
                    when_true.backing(),
                    when_false.backing(),
                ]),
                PortableExpressionOperation::Tuple(v)
                | PortableExpressionOperation::Collection(v) => v.backing(),
                PortableExpressionOperation::Record(v) => v.backing(),
                PortableExpressionOperation::Variant { tag, payload } => {
                    sum([tag.backing(), payload.backing()])
                }
                PortableExpressionOperation::SemanticCall { kind, arguments } => {
                    sum([kind.backing(), arguments.backing()])
                }
            },
        ])
    }
}
