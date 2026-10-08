//! Conservative construction proof: exact forwarded refinements or constants.
//! Arithmetic and cross-field laws still require a separate admitted validator.
use super::*;
use crate::PortableExpressionProjection;
use alloc::string::String;

pub(super) fn proves(
    node: &PortableExpressionNode,
    input: &conduit_core::StructuredInfoType,
    native: &CheckedNativeType,
    types: &[CheckedNativeType],
) -> bool {
    if node.value_type != native.value_type || !native.invariants.is_empty() {
        return false;
    }
    native.value_contracts.iter().all(|required| {
        proves_at(
            node,
            &required.representation_path,
            &required.contract,
            input,
            types,
        )
    })
}

fn proves_at(
    node: &PortableExpressionNode,
    path: &str,
    required: &conduit_core::CheckedValueContract,
    input: &conduit_core::StructuredInfoType,
    types: &[CheckedNativeType],
) -> bool {
    let Ok(selected) = constructed_member(node, path) else {
        return false;
    };
    let Some((value, remaining)) = selected else {
        return true;
    };
    if let Some(rest) = remaining.strip_prefix("[]") {
        if let Op::Collection(values) = &value.operation {
            return values
                .iter()
                .all(|item| proves_at(item, rest, required, input, types));
        }
    }
    if let Op::Conditional {
        when_true,
        when_false,
        ..
    } = &value.operation
    {
        return proves_at(when_true, remaining, required, input, types)
            && proves_at(when_false, remaining, required, input, types);
    }
    if remaining.is_empty() && matches!(value.operation, Op::Literal(_)) {
        let constant = PortableExpressionProgram {
            input_type: conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
                conduit_core::UNIT_INFO_ID,
            ))
            .expect("Unit"),
            output_type: value.value_type.clone(),
            root: value.clone(),
        };
        return constant.evaluate(&[]).is_ok_and(|bytes| {
            if matches!(value.value_type.shape(), StructuredInfoTypeShape::Leaf(_)) {
                return required.validate(&bytes).is_ok();
            }
            conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes).is_ok_and(|value| {
                let conduit_core::StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                    return false;
                };
                required.validate(bytes).is_ok()
            })
        });
    }
    let Some(mut path) = input_path(value) else {
        return false;
    };
    path.push_str(remaining);
    retained_contract(input, &path, required, types)
}

// A typed container can forward a checked native member without flattening
// that member's laws into the container. Only exact retained native metadata
// establishes a refinement; matching a primitive shape alone is insufficient.
fn retained_contract(
    input: &conduit_core::StructuredInfoType,
    path: &str,
    required: &conduit_core::CheckedValueContract,
    types: &[CheckedNativeType],
) -> bool {
    if types.iter().filter(|ty| &ty.value_type == input).any(|ty| {
        ty.value_contracts
            .iter()
            .any(|contract| contract.representation_path == path && &contract.contract == required)
    }) {
        return true;
    }
    let Some(rest) = path.strip_prefix('.') else {
        return false;
    };
    let end = rest.find(['.', '|', '[', '?']).unwrap_or(rest.len());
    let (field, remaining) = rest.split_at(end);
    let StructuredInfoTypeShape::Record { fields, .. } = input.shape() else {
        return false;
    };
    fields
        .iter()
        .find(|candidate| candidate.name() == field)
        .is_some_and(|field| retained_contract(field.value_type(), remaining, required, types))
}

fn constructed_member<'a>(
    node: &'a PortableExpressionNode,
    path: &'a str,
) -> Result<Option<(&'a PortableExpressionNode, &'a str)>, ()> {
    if path.is_empty()
        || path.starts_with("[]")
        || matches!(
            node.operation,
            Op::Input | Op::Projection { .. } | Op::Conditional { .. }
        )
    {
        return Ok(Some((node, path)));
    }
    if let Some(rest) = path.strip_prefix('|') {
        let end = rest.find(['.', '|', '[', '?']).unwrap_or(rest.len());
        let (case, remaining) = rest.split_at(end);
        let Op::Variant { tag, payload } = &node.operation else {
            return Err(());
        };
        return if tag == case {
            constructed_member(payload, remaining)
        } else {
            Ok(None)
        };
    }
    let rest = path.strip_prefix('.').ok_or(())?;
    let end = rest.find(['.', '|', '[', '?']).unwrap_or(rest.len());
    let (field, remaining) = rest.split_at(end);
    let Op::Record(fields) = &node.operation else {
        return Err(());
    };
    let value = &fields.iter().find(|(name, _)| name == field).ok_or(())?.1;
    constructed_member(value, remaining)
}

fn input_path(node: &PortableExpressionNode) -> Option<String> {
    match &node.operation {
        Op::Input => Some(String::new()),
        Op::Projection {
            value,
            member: PortableExpressionProjection::Field(field),
        } => {
            let mut path = input_path(value)?;
            match value.value_type.shape() {
                StructuredInfoTypeShape::Variant { cases, .. } => {
                    if !cases.iter().any(|case| case.tag() == field) {
                        return None;
                    }
                    path.push('|');
                }
                _ => path.push('.'),
            }
            path.push_str(field);
            Some(path)
        }
        Op::Projection {
            value,
            member: PortableExpressionProjection::TupleIndex(index),
        } => {
            let StructuredInfoTypeShape::Record { fields, .. } = value.value_type.shape() else {
                return None;
            };
            let canonical = conduit_core::tuple_info_type(
                fields
                    .iter()
                    .map(|field| field.value_type().clone())
                    .collect(),
            )
            .ok()?;
            if canonical != value.value_type {
                return None;
            }
            let field = fields.get(usize::from(*index))?;
            let mut path = input_path(value)?;
            path.push('.');
            path.push_str(field.name());
            Some(path)
        }
        _ => None,
    }
}
