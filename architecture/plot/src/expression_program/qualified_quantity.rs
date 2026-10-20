//! Physical expression meaning is distinct from its qualified evidence carrier.
use super::*;
use conduit_core::{CheckedValueContract, ValueConstraint};

pub(super) fn checked_node(
    checked: &CheckedExpression,
    value: PortableExpressionNode,
) -> PortableExpressionNode {
    PortableExpressionNode {
        value_type: checked
            .value_type
            .structured_info_type_with(&checked.semantic_structures)
            .expect("checked qualified output Type"),
        operation: PortableExpressionOperation::QualifiedQuantity {
            input: checked
                .input_value_contract
                .clone()
                .expect("qualified input"),
            output: checked
                .output_value_contract
                .clone()
                .expect("qualified output"),
            physical_input: checked
                .qualified_quantity_input
                .as_ref()
                .expect("qualified physical input")
                .structured_info_type_with(&checked.semantic_structures)
                .expect("checked physical input Type"),
            value: Box::new(value),
        },
    }
}

pub(super) fn validate(
    program: &PortableExpressionProgram,
) -> Result<(), PortableExpressionProgramRefusal> {
    let invalid = PortableExpressionProgramRefusal::MalformedEncoding;
    let PortableExpressionOperation::QualifiedQuantity {
        input,
        output,
        physical_input,
        value,
    } = &program.root.operation
    else {
        return Err(invalid);
    };
    if program.root.value_type != program.output_type {
        return Err(invalid);
    }
    fn physical_kind(contract: &CheckedValueContract) -> Option<&conduit_core::KindId> {
        let [ValueConstraint::NearQuantity { source_kind, .. }] = contract.constraints.as_slice()
        else {
            return None;
        };
        crate::expression_quantity::is_quantity(source_kind.as_str()).then_some(source_kind)
    }
    if Some(&crate::expression_definition::expression_port_kind(
        physical_input,
    )?) != physical_kind(input)
        || Some(&crate::expression_definition::expression_port_kind(
            &value.value_type,
        )?) != physical_kind(output)
    {
        return Err(invalid);
    }
    fn body(node: &PortableExpressionNode, input: &StructuredInfoType) -> bool {
        use crate::{BinaryOperator as B, UnaryOperator as U};
        match &node.operation {
            PortableExpressionOperation::Input => &node.value_type == input,
            PortableExpressionOperation::Literal(_)
            | PortableExpressionOperation::CanonicalLiteral(_)
            | PortableExpressionOperation::Constant(_) => true,
            PortableExpressionOperation::Unary {
                operator: U::Negate,
                operand,
            } => body(operand, input),
            PortableExpressionOperation::Binary {
                operator: B::Add | B::Subtract | B::Multiply | B::Divide,
                left,
                right,
                ..
            } => body(left, input) && body(right, input),
            _ => false,
        }
    }
    if !body(value, physical_input) {
        return Err(invalid);
    }
    Ok(())
}
