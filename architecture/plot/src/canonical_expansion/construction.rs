//! Refuse newly constructed refined values until their laws have an admitted
//! executable validator. Shape checking alone cannot establish those laws.
use super::*;
use crate::{
    CheckedNativeType, PortableExpressionNode, PortableExpressionOperation as Op,
    PortableExpressionProgram,
};
use conduit_core::StructuredInfoTypeShape;

pub(super) fn validate(
    gears: &[CheckedGear],
    types: &[CheckedNativeType],
) -> Result<(), CanonicalExpansionDiagnostic> {
    for gear in gears {
        if gear.kind_contract_revision.as_str() != crate::PURE_EXPRESSION_REVISION {
            continue;
        }
        let Some(ConfigurationValue::Text(encoded)) = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "program")
            .map(|entry| &entry.value)
        else {
            continue;
        };
        let program =
            PortableExpressionProgram::from_canonical_hex(encoded).map_err(|_| refusal())?;
        validate_node(&program.root, types)?;
    }
    Ok(())
}

fn validate_node(
    node: &PortableExpressionNode,
    types: &[CheckedNativeType],
) -> Result<(), CanonicalExpansionDiagnostic> {
    if matches!(
        node.operation,
        Op::Record(_) | Op::Variant { .. } | Op::Literal(_)
    ) {
        let native = types.iter().find(|ty| ty.value_type == node.value_type);
        let is_native = match node.value_type.shape() {
            StructuredInfoTypeShape::Record { schema, .. }
            | StructuredInfoTypeShape::Variant { schema, .. }
            | StructuredInfoTypeShape::Nominal { schema, .. } => {
                schema.as_str().starts_with("type/")
            }
            _ => false,
        };
        // Imported shapes alone do not retain refinement/where-law metadata.
        // Passing their existing values is legal; constructing one must refuse.
        if native.is_some_and(|ty| !ty.invariants.is_empty() || !ty.value_contracts.is_empty())
            || (is_native && native.is_none())
        {
            if matches!(node.operation, Op::Literal(_)) {
                if let Some(native) = native {
                    // A closed constant can establish its own laws before Play.
                    // It must not disable the existing proof of input arithmetic.
                    let constant = PortableExpressionProgram {
                        input_type: conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
                            conduit_core::UNIT_INFO_ID,
                        ))
                        .map_err(|_| refusal())?,
                        output_type: node.value_type.clone(),
                        root: node.clone(),
                    };
                    let bytes = constant.evaluate(&[]).map_err(|_| refusal())?;
                    let value = conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes)
                        .map_err(|_| refusal())?;
                    crate::rust_binding::validate_native_contracts(&value, &native.value_contracts)
                        .and_then(|()| {
                            crate::rust_binding::validate_native_invariants(
                                &value,
                                &native.invariants,
                            )
                        })
                        .map_err(|_| refusal())?;
                    return Ok(());
                }
            }
            return Err(refusal());
        }
    }
    match &node.operation {
        Op::Projection { value, .. } | Op::Unary { operand: value, .. } => {
            validate_node(value, types)?
        }
        Op::Binary { left, right, .. } => {
            validate_node(left, types)?;
            validate_node(right, types)?;
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            validate_node(condition, types)?;
            validate_node(when_true, types)?;
            validate_node(when_false, types)?;
        }
        Op::Record(fields) => {
            for (_, value) in fields {
                validate_node(value, types)?;
            }
        }
        Op::Tuple(values)
        | Op::Collection(values)
        | Op::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                validate_node(value, types)?;
            }
        }
        Op::Variant { payload, .. } => validate_node(payload, types)?,
        Op::Input | Op::Literal(_) => {}
    }
    Ok(())
}

fn refusal() -> CanonicalExpansionDiagnostic {
    CanonicalExpansionDiagnostic::new(
        "CND-FRM-046",
        "pure construction of a refined native Type requires an admitted law validator".into(),
    )
}
