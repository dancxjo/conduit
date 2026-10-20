//! Source-owned reversible Boolean facts, shared by every Mask.
use crate::{Face, FacePropertyValue};

/// Availability is separate. Both Boolean replacements and one exact Source
/// association are required; a one-way action never becomes a toggle.
pub fn reversible_boolean_value(face: &Face, action_index: usize) -> Option<bool> {
    let action = face.actions.get(action_index)?;
    if action.arguments.len() != 1 {
        return None;
    }
    let argument = &action.arguments[0];
    if argument.contract.value_kind.as_str() != "value/bool"
        || argument.contract.maximum_bytes != 1
        || argument.contract.validate(&[0]).is_err()
        || argument.contract.validate(&[1]).is_err()
    {
        return None;
    }
    let mut values = face
        .properties
        .iter()
        .filter(|property| property.subject == action.target && property.name == "selected");
    let FacePropertyValue::Flag(value) = &values.next()?.value else {
        return None;
    };
    if values.next().is_some() {
        return None;
    }
    let mut bindings = face.properties.iter().filter(|property| {
        property.subject == action.target && property.name == "boolean-change-action"
    });
    let FacePropertyValue::Identity(identity) = &bindings.next()?.value else {
        return None;
    };
    if identity != &action.identity || bindings.next().is_some() {
        return None;
    }
    Some(*value)
}
