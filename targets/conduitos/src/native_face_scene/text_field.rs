//! Read current text through an exact Source value/action association.
use conduit_face::{Face, FacePropertyValue, FaceRole, UTF8_TEXT_VALUE_KIND};

fn property<'a>(face: &'a Face, subject: &str, name: &str) -> Option<&'a FacePropertyValue> {
    let mut values = face
        .properties
        .iter()
        .filter(|value| value.subject == subject && value.name == name);
    let value = &values.next()?.value;
    values.next().is_none().then_some(value)
}

/// Availability stays separate: an unavailable field can still disclose its
/// current value. This grants no input custody and never copies application state.
pub(super) fn current(face: &Face, action_index: usize) -> Option<&str> {
    let action = face.actions.get(action_index)?;
    let [argument] = action.arguments.as_slice() else {
        return None;
    };
    if argument.contract.value_kind.as_str() != UTF8_TEXT_VALUE_KIND
        || !face
            .subjects
            .iter()
            .any(|subject| subject.identity == action.target && subject.role == FaceRole::TextEntry)
    {
        return None;
    }
    let FacePropertyValue::Identity(binding) =
        property(face, &action.target, "value-change-action")?
    else {
        return None;
    };
    if binding != &action.identity {
        return None;
    }
    let FacePropertyValue::TypedValue { contract, bytes } =
        property(face, &action.target, "current-value")?
    else {
        return None;
    };
    if contract != &argument.contract || contract.validate(bytes).is_err() {
        return None;
    }
    core::str::from_utf8(bytes).ok()
}
