//! Refuse newly constructed refined values until their laws have an admitted
//! executable validator. Shape checking alone cannot establish those laws.
use super::*;
use crate::{
    CheckedNativeType, PortableExpressionNode, PortableExpressionOperation as Op,
    PortableExpressionProgram,
};
use conduit_core::StructuredInfoTypeShape;
mod refinement;

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
        validate_node(&program.root, &program.input_type, types)?;
    }
    Ok(())
}

fn validate_node(
    node: &PortableExpressionNode,
    input_type: &conduit_core::StructuredInfoType,
    types: &[CheckedNativeType],
) -> Result<(), CanonicalExpansionDiagnostic> {
    if matches!(
        node.operation,
        Op::Record(_) | Op::Variant { .. } | Op::Collection(_) | Op::Literal(_)
    ) {
        let is_native = match node.value_type.shape() {
            StructuredInfoTypeShape::Record { schema, .. }
            | StructuredInfoTypeShape::Variant { schema, .. }
            | StructuredInfoTypeShape::Nominal { schema, .. } => {
                schema.as_str().starts_with("type/")
            }
            _ => false,
        };
        let native = types
            .iter()
            .find(|ty| ty.value_type == node.value_type)
            .or_else(|| {
                is_native
                    .then(|| {
                        types
                            .iter()
                            .find(|ty| contains_type(&ty.value_type, &node.value_type))
                    })
                    .flatten()
            });
        // Imported shapes alone do not retain refinement/where-law metadata.
        // Passing their existing values is legal; constructing one must refuse.
        if (native.is_some_and(|ty| !ty.invariants.is_empty() || !ty.value_contracts.is_empty())
            || (is_native && native.is_none()))
            && !native.is_some_and(|native| refinement::proves(node, input_type, native, types))
        {
            let mut admitted_closed = false;
            if closed(node) {
                if let Some(native) = native.filter(|ty| ty.value_type == node.value_type) {
                    // A closed constant can establish its own laws before Play.
                    // It must not disable the existing proof of input arithmetic.
                    let constant = PortableExpressionProgram {
                        input_type: conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
                            conduit_core::EMPTY_INFO_ID,
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
                    // The parent's own laws do not establish nested Types'
                    // where laws. Continue through every constructed child.
                    admitted_closed = true;
                }
            }
            if !admitted_closed {
                return Err(refusal());
            }
        }
    }
    match &node.operation {
        Op::Projection { value, .. } | Op::Unary { operand: value, .. } => {
            validate_node(value, input_type, types)?
        }
        Op::Binary { left, right, .. } => {
            validate_node(left, input_type, types)?;
            validate_node(right, input_type, types)?;
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            validate_node(condition, input_type, types)?;
            validate_node(when_true, input_type, types)?;
            validate_node(when_false, input_type, types)?;
        }
        Op::Record(fields) => {
            for (_, value) in fields {
                validate_node(value, input_type, types)?;
            }
        }
        Op::Tuple(values)
        | Op::Collection(values)
        | Op::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                validate_node(value, input_type, types)?;
            }
        }
        Op::Variant { payload, .. } => validate_node(payload, input_type, types)?,
        Op::Input | Op::Literal(_) => {}
    }
    Ok(())
}

// Closed authored constructors can execute the same Native laws at preparation
// time. An input-dependent expression cannot use this route to evade proof.
fn closed(node: &PortableExpressionNode) -> bool {
    match &node.operation {
        Op::Literal(_) => true,
        Op::Record(fields) => fields.iter().all(|(_, value)| closed(value)),
        Op::Tuple(values) | Op::Collection(values) => values.iter().all(closed),
        Op::Variant { payload, .. } => closed(payload),
        _ => false,
    }
}

// Inline record payloads have their own exact schema but retain the owning
// native declaration's law metadata. Do not treat them as unknown imports.
fn contains_type(
    root: &conduit_core::StructuredInfoType,
    target: &conduit_core::StructuredInfoType,
) -> bool {
    if root == target {
        return true;
    }
    match root.shape() {
        StructuredInfoTypeShape::Record { fields, .. } => fields
            .iter()
            .any(|field| contains_type(field.value_type(), target)),
        StructuredInfoTypeShape::Variant { cases, .. } => cases
            .iter()
            .any(|case| contains_type(case.payload_type(), target)),
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => contains_type(element, target),
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            contains_type(representation, target)
        }
        StructuredInfoTypeShape::Leaf(_) => false,
    }
}

fn refusal() -> CanonicalExpansionDiagnostic {
    CanonicalExpansionDiagnostic::new(
        "CND-FRM-046",
        "pure construction of a refined native Type requires an admitted law validator".into(),
    )
}
