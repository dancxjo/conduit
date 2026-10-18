//! Refuse portable annotations for which the reference Back retains no witness.
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape as Shape};
use conduit_plot::{
    PortableExpressionNode, PortableExpressionOperation as Op, PortableExpressionProgram,
};

pub(super) fn check(program: &PortableExpressionProgram) -> Result<(), String> {
    ty(&program.input_type)?;
    ty(&program.output_type)?;
    node(&program.root)
}
fn ty(value: &StructuredInfoType) -> Result<(), String> {
    match value.shape() {
        Shape::Leaf(_) => Ok(()),
        Shape::Nominal { .. } => {
            Err("std/source-expression/native-law-witness-unavailable@1".into())
        }
        Shape::Record { fields, .. } => fields.iter().try_for_each(|field| ty(field.value_type())),
        Shape::Variant { cases, .. } => cases.iter().try_for_each(|case| ty(case.payload_type())),
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => ty(element),
    }
}
fn node(value: &PortableExpressionNode) -> Result<(), String> {
    ty(&value.value_type)?;
    match &value.operation {
        Op::Input | Op::Literal(_) | Op::CanonicalLiteral(_) | Op::Constant(_) => Ok(()),
        Op::Projection { value, .. }
        | Op::Unary { operand: value, .. }
        | Op::Variant { payload: value, .. } => node(value),
        Op::Binary {
            proven,
            left,
            right,
            ..
        } => {
            if *proven {
                return Err("std/source-expression/arithmetic-witness-unavailable@1".into());
            }
            node(left)?;
            node(right)
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            node(condition)?;
            node(when_true)?;
            node(when_false)
        }
        Op::Tuple(values)
        | Op::Collection(values)
        | Op::SemanticCall {
            arguments: values, ..
        } => values.iter().try_for_each(node),
        Op::Record(fields) => fields.iter().try_for_each(|(_, value)| node(value)),
    }
}
