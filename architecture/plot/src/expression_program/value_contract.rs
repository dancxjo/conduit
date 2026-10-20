//! Existing checked predicates retained with the exact expression computation.
use super::*;
use conduit_core::CheckedValueContract;

pub(super) fn checked_node(
    checked: &CheckedExpression,
    value: PortableExpressionNode,
) -> PortableExpressionNode {
    if checked.qualified_quantity_input.is_some() {
        return super::qualified_quantity::checked_node(checked, value);
    }
    if checked.input_value_contract.is_none() && checked.output_value_contract.is_none() {
        return value;
    }
    PortableExpressionNode {
        value_type: value.value_type.clone(),
        operation: PortableExpressionOperation::ValueContract {
            input: checked.input_value_contract.clone(),
            output: checked.output_value_contract.clone(),
            value: Box::new(value),
        },
    }
}
pub(super) fn push_contract(sink: &mut ProgramSink, contract: Option<&CheckedValueContract>) {
    sink.push(u8::from(contract.is_some()));
    if let Some(contract) = contract {
        let bytes = contract.identity_bytes();
        push_len(sink, bytes.len());
        sink.extend_from_slice(&bytes);
    }
}

pub(crate) fn retain_laws(
    laws: &mut Vec<conduit_core::KindSemanticLaw>,
    input: Option<&CheckedValueContract>,
    output: Option<&CheckedValueContract>,
) {
    use conduit_core::{FrontValueContract, FrontValueLocation, KindSemanticLaw};
    if input.is_none() && output.is_none() {
        return;
    }
    let index = laws
        .iter()
        .position(|law| matches!(law, KindSemanticLaw::ValueContracts(_)))
        .unwrap_or_else(|| {
            laws.push(KindSemanticLaw::ValueContracts(Vec::new()));
            laws.len() - 1
        });
    let KindSemanticLaw::ValueContracts(contracts) = &mut laws[index] else {
        unreachable!()
    };
    for (location, contract) in [
        (
            FrontValueLocation::Input(conduit_core::port_id("input")),
            input,
        ),
        (
            FrontValueLocation::Output(conduit_core::port_id("output")),
            output,
        ),
    ] {
        if let Some(contract) = contract {
            if let Some(existing) = contracts.iter_mut().find(|c| c.location == location) {
                existing.contract = contract.clone();
            } else {
                contracts.push(FrontValueContract {
                    location,
                    contract: contract.clone(),
                });
            }
        }
    }
}

pub(crate) fn boundary(
    node: &PortableExpressionNode,
) -> (
    Option<&CheckedValueContract>,
    Option<&CheckedValueContract>,
    &PortableExpressionNode,
) {
    match &node.operation {
        PortableExpressionOperation::QualifiedQuantity {
            input,
            output,
            value,
            ..
        } => (Some(input), Some(output), value),
        PortableExpressionOperation::ValueContract {
            input,
            output,
            value,
        } => (input.as_ref(), output.as_ref(), value),
        _ => (None, None, node),
    }
}

pub(crate) fn validate(
    program: &PortableExpressionProgram,
) -> Result<(), PortableExpressionProgramRefusal> {
    let (input, output, body) = boundary(&program.root);
    if matches!(
        program.root.operation,
        PortableExpressionOperation::QualifiedQuantity { .. }
    ) {
        super::qualified_quantity::validate(program)?;
    } else if body.value_type != program.output_type {
        return Err(PortableExpressionProgramRefusal::MalformedEncoding);
    }
    for (contract, ty) in [(input, &program.input_type), (output, &program.output_type)] {
        if let Some(contract) = contract {
            if contract.value_kind != crate::expression_definition::expression_port_kind(ty)? {
                return Err(PortableExpressionProgramRefusal::MalformedEncoding);
            }
            contract
                .validate_definition()
                .map_err(|_| PortableExpressionProgramRefusal::MalformedEncoding)?;
        }
    }
    Ok(())
}
