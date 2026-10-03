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
        let Some(value) = constructed_member(node, &required.representation_path) else {
            return false;
        };
        if matches!(value.operation, Op::Literal(_)) {
            let constant = PortableExpressionProgram {
                input_type: conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
                    conduit_core::UNIT_INFO_ID,
                ))
                .expect("Unit"),
                output_type: value.value_type.clone(),
                root: value.clone(),
            };
            return constant
                .evaluate(&[])
                .is_ok_and(|bytes| required.contract.validate(&bytes).is_ok());
        }
        let Some(path) = input_path(value) else {
            return false;
        };
        types.iter().filter(|ty| &ty.value_type == input).any(|ty| {
            ty.value_contracts.iter().any(|contract| {
                contract.representation_path == path && contract.contract == required.contract
            })
        })
    })
}

fn constructed_member<'a>(
    node: &'a PortableExpressionNode,
    path: &str,
) -> Option<&'a PortableExpressionNode> {
    if path.is_empty() {
        return Some(node);
    }
    let rest = path.strip_prefix('.')?;
    let end = rest.find(['.', '|', '[', '?']).unwrap_or(rest.len());
    let (field, remaining) = rest.split_at(end);
    let Op::Record(fields) = &node.operation else {
        return None;
    };
    let value = &fields.iter().find(|(name, _)| name == field)?.1;
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
            path.push('.');
            path.push_str(field);
            Some(path)
        }
        _ => None,
    }
}
