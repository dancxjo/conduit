//! Read current text through an exact Source value/action association.
use conduit_core::ValueConstraint;
use conduit_face::{Face, FacePropertyValue, FaceRole, UTF8_TEXT_VALUE_KIND};

// Mask preparation bounds, independent of the Source's allowed value set.
const MAX_CHOICES: usize = 32;
const MAX_CHOICE_BYTES: usize = 2_048;

pub(super) struct Choices<'a> {
    values: [&'a str; MAX_CHOICES],
    count: usize,
}

impl<'a> Choices<'a> {
    pub(super) fn as_slice(&self) -> &[&'a str] {
        &self.values[..self.count]
    }
}

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

/// Choose only from the same small checked membership as the Source action.
/// The returned bytes are a proposed replacement, never a Source update.
pub(super) fn adjacent<'a>(
    face: &'a Face,
    action_index: usize,
    draft: Option<&[u8]>,
    forward: bool,
) -> Option<&'a str> {
    let committed = current(face, action_index).unwrap_or("");
    let choices = choices(face, action_index)?;
    let contract = &face.actions[action_index].arguments[0].contract;
    let choices = choices.as_slice();
    let value = draft
        .and_then(|bytes| core::str::from_utf8(bytes).ok())
        .unwrap_or(committed);
    let next = choices
        .iter()
        .position(|choice| *choice == value)
        .map_or_else(
            || if forward { 0 } else { choices.len() - 1 },
            |index| {
                if forward {
                    (index + 1) % choices.len()
                } else {
                    (index + choices.len() - 1) % choices.len()
                }
            },
        );
    let selected = choices[next];
    contract.validate(selected.as_bytes()).ok()?;
    Some(selected)
}

/// Borrow a closed, legible enumeration without discarding other constraints.
/// Both a committed field and a provisional action argument can offer choices.
/// Current-value reading still requires the exact Source association above.
/// Unsupported/larger sets keep their complete Details and typed input path.
pub(super) fn choices(face: &Face, action_index: usize) -> Option<Choices<'_>> {
    let action = face.actions.get(action_index)?;
    let [argument] = action.arguments.as_slice() else {
        return None;
    };
    let contract = &argument.contract;
    if contract.value_kind.as_str() != UTF8_TEXT_VALUE_KIND {
        return None;
    }
    let mut sets = contract
        .constraints
        .iter()
        .filter_map(|constraint| match constraint {
            ValueConstraint::CanonicalMembership {
                members,
                negated: false,
            } => Some(members),
            _ => None,
        });
    let members = sets.next()?;
    if sets.next().is_some() || members.is_empty() || members.len() > MAX_CHOICES {
        return None;
    }
    let mut bytes = 0usize;
    let mut choices = Choices {
        values: [""; MAX_CHOICES],
        count: 0,
    };
    for member in members {
        bytes = bytes.checked_add(member.len())?;
        if bytes > MAX_CHOICE_BYTES
            || member.is_empty()
            || !member.iter().all(|byte| (32..=126).contains(byte))
        {
            return None;
        }
        if contract.validate(member).is_ok() {
            choices.values[choices.count] = core::str::from_utf8(member).ok()?;
            choices.count += 1;
        }
    }
    (choices.count != 0).then_some(choices)
}
