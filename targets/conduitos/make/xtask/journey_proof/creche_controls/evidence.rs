//! Source/Show evidence only; never reconstructs a renderer or owner state.
use super::*;
use conduit_core::ValueConstraint;
use conduit_face::{Face, FaceInteraction, FacePropertyValue};
pub(super) const FOCUS: &str = "CONDUIT_CRECHE_FOCUS ";
pub(super) const CHANGE: &str = "CONDUIT_CRECHE_CHANGE ";

pub(super) fn text(path: &Path) -> Result<String, ConduitosError> {
    let bytes = fs::read(path).map_err(io_error)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(refusal("controls serial exceeds existing bound"));
    }
    Ok(journey_input::complete_records(&bytes)?.into())
}
pub(super) fn rows(text: &str, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
    face_correspondence::records(text, prefix)
}
pub(super) struct Snapshot {
    pub face: Face,
    pub record: Value,
    pub focus: Value,
}
pub(super) fn snapshot(text: &str) -> Result<Snapshot, ConduitosError> {
    let record = rows(text, "CONDUIT_NATIVE_FACE ")?
        .pop()
        .ok_or_else(|| refusal("current Face absent"))?;
    let face: Face =
        serde_json::from_value(record["face"].clone()).map_err(|e| refusal(e.to_string()))?;
    face.validate().map_err(|e| refusal(e.to_string()))?;
    super::super::look_selection::validate_current(&record)?;
    super::custody::component(&record)?;
    let boot = super::super::journey_records::boot(text)?
        .ok_or_else(|| refusal("controls Boot absent"))?;
    if record["show"]["host_id"] != boot["host_id"] || record["show"]["boot_id"] != boot["boot_id"]
    {
        return Err(refusal("current control Show differs from actual Boot"));
    }
    let focus = rows(text, FOCUS)?
        .pop()
        .ok_or_else(|| refusal("current focus absent"))?;
    focus_custody(&record, &focus, &boot)?;
    if face.basis.body_id.is_some() {
        return Err(refusal("controls need zero Body"));
    }
    Ok(Snapshot {
        face,
        record,
        focus,
    })
}
fn focus_custody(record: &Value, focus: &Value, boot: &Value) -> Result<(), ConduitosError> {
    let show = &record["show"];
    let face = &record["face"];
    if record["schema"] != "conduit.native-face/v1"
        || show["lifecycle"] != "Available"
        || show["face_id"] != face["identity"]
        || show["face_revision"] != face["revision"]
        || focus["schema"] != "conduit.creche-focus/v1"
        || focus["face_id"] != face["identity"]
        || focus["face_revision"] != face["revision"]
        || focus["show_id"] != show["show_id"]
        || show["show_id"].as_str().is_none_or(str::is_empty)
        || show["host_id"] != boot["host_id"]
        || show["boot_id"] != boot["boot_id"]
    {
        return Err(refusal(
            "focus lacks current acknowledged Face/Show/Host/Boot custody",
        ));
    }
    Ok(())
}
pub(super) fn current(face: &Face, action: &str) -> Result<Vec<u8>, ConduitosError> {
    let a = face
        .actions
        .iter()
        .find(|a| a.identity == action)
        .ok_or_else(|| refusal("field action absent"))?;
    let [arg] = a.arguments.as_slice() else {
        return Err(refusal("field needs one exact argument"));
    };
    if arg.contract.value_kind.as_str() != conduit_face::UTF8_TEXT_VALUE_KIND
        || !face.subjects.iter().any(|subject| {
            subject.identity == a.target && subject.role == conduit_face::FaceRole::TextEntry
        })
    {
        return Err(refusal("unsupported text field realization"));
    }
    let values = face
        .properties
        .iter()
        .filter(|p| p.subject == a.target && p.name == "current-value")
        .collect::<Vec<_>>();
    let [property] = values.as_slice() else {
        return Err(refusal("field current value ambiguous"));
    };
    let FacePropertyValue::TypedValue { contract, bytes } = &property.value else {
        return Err(refusal("field lacks typed current value"));
    };
    if contract != &arg.contract || contract.validate(bytes).is_err() {
        return Err(refusal("field value differs from action contract"));
    }
    if !face.properties.iter().any(|p| {
        p.subject == a.target
            && p.name == "value-change-action"
            && p.value == FacePropertyValue::Identity(action.into())
    }) {
        return Err(refusal("field action association absent"));
    }
    Ok(bytes.clone())
}
pub(super) fn members(face: &Face) -> Result<Vec<Vec<u8>>, ConduitosError> {
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == "creche.naming")
        .ok_or_else(|| refusal("naming action absent"))?;
    if !action.target.ends_with("/name-system")
        || !action.availability.is_available()
        || action.arguments.len() != 1
    {
        return Err(refusal(
            "naming field not current available canonical target",
        ));
    }
    let contract = &action.arguments[0].contract;
    let sets = contract
        .constraints
        .iter()
        .filter_map(|c| match c {
            ValueConstraint::CanonicalMembership {
                members,
                negated: false,
            } => Some(members),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [set] = sets.as_slice() else {
        return Err(refusal("naming needs exact positive membership"));
    };
    if set.is_empty() || set.len() > 32 || set.iter().map(Vec::len).sum::<usize>() > 2048 {
        return Err(refusal("naming membership exceeds existing Mask bound"));
    }
    let values = set
        .iter()
        .filter(|v| contract.validate(v).is_ok())
        .cloned()
        .collect::<Vec<_>>();
    if !values.iter().any(|v| v == b"welsh") {
        return Err(refusal("actual naming membership lacks welsh"));
    }
    Ok(values)
}
/// Boolean fields have one checked reversible control, not a second text input.
/// The original full action/argument inventory is retained independently.
pub(super) fn controls(face: &Face) -> Result<Vec<(String, Option<String>)>, ConduitosError> {
    let mut out = Vec::new();
    for a in face
        .actions
        .iter()
        .filter(|a| a.availability.is_available())
    {
        out.push((a.identity.clone(), None));
        if a.arguments.is_empty() {
            continue;
        }
        let [arg] = a.arguments.as_slice() else {
            return Err(refusal("unsupported multi-argument focus realization"));
        };
        if arg.contract.value_kind.as_str() == "value/bool" {
            let selected = face
                .properties
                .iter()
                .filter(|p| p.subject == a.target && p.name == "selected")
                .collect::<Vec<_>>();
            let bindings = face
                .properties
                .iter()
                .filter(|p| p.subject == a.target && p.name == "boolean-change-action")
                .collect::<Vec<_>>();
            if arg.contract.maximum_bytes != 1
                || arg.contract.validate(&[0]).is_err()
                || arg.contract.validate(&[1]).is_err()
                || selected.len() != 1
                || !matches!(selected[0].value, FacePropertyValue::Flag(_))
                || bindings.len() != 1
                || bindings[0].value != FacePropertyValue::Identity(a.identity.clone())
            {
                return Err(refusal("unsupported Boolean combined-control association"));
            }
        } else if arg.contract.value_kind.as_str() == conduit_face::UTF8_TEXT_VALUE_KIND {
            current(face, &a.identity)?;
            out.push((a.identity.clone(), Some(arg.name.clone())));
        } else {
            return Err(refusal("unsupported native focus argument kind"));
        }
    }
    Ok(out)
}
pub(super) fn key(f: &Value) -> Result<(String, Option<String>), ConduitosError> {
    let action = f["action_id"]
        .as_str()
        .ok_or_else(|| refusal("focus action absent"))?
        .into();
    let argument = if f["argument_name"].is_null() {
        None
    } else {
        Some(
            f["argument_name"]
                .as_str()
                .ok_or_else(|| refusal("invalid focus argument"))?
                .into(),
        )
    };
    Ok((action, argument))
}
pub(super) fn unchanged(initial: &Snapshot, next: &Snapshot) -> Result<(), ConduitosError> {
    if initial.face != next.face || initial.record["show"] != next.record["show"] {
        return Err(refusal("navigation changed canonical Source Face/Show"));
    }
    Ok(())
}
pub(super) fn validate_change(
    text: &str,
    before: &Snapshot,
    action_id: &str,
    value: &[u8],
    count: usize,
) -> Result<Value, ConduitosError> {
    let changes = rows(text, CHANGE)?;
    if changes.len() != count + 1 {
        return Err(refusal(
            "one checked input must produce exactly one Source change",
        ));
    }
    let c = changes.last().unwrap();
    let after = snapshot(text)?;
    let i: FaceInteraction =
        serde_json::from_value(c["interaction"].clone()).map_err(|e| refusal(e.to_string()))?;
    let a = before
        .face
        .actions
        .iter()
        .find(|a| a.identity == action_id && a.availability.is_available())
        .ok_or_else(|| refusal("checked action unavailable"))?;
    let [arg] = a.arguments.as_slice() else {
        return Err(refusal("checked field argument absent"));
    };
    if c["schema"] != "conduit.creche-change/v1"
        || c["disposition"] != "source-accepted-and-shown"
        || i.face_id != before.face.identity.as_str()
        || i.face_revision != before.face.revision
        || json!(i.show_id) != before.record["show"]["show_id"]
        || i.action_id != a.identity
        || i.target != a.target
        || i.arguments.len() != 1
        || i.arguments[0].name != arg.name
        || i.arguments[0].value_kind != arg.contract.value_kind.as_str()
        || i.arguments[0].value != value
        || arg.contract.validate(value).is_err()
        || c["before_face_id"] != json!(before.face.identity)
        || c["before_revision"] != before.face.revision
        || c["before_show_id"] != before.record["show"]["show_id"]
        || c["after_face_id"] != json!(after.face.identity)
        || c["after_revision"] != after.face.revision
        || c["after_show_id"] != after.record["show"]["show_id"]
        || after.face.revision <= before.face.revision
        || after.face.identity == before.face.identity
        || before.face.basis != after.face.basis
        || before.record["show"]["show_id"] == after.record["show"]["show_id"]
        || before.record["show"]["mask_plot"] != after.record["show"]["mask_plot"]
        || ["host_id", "boot_id", "offer_generation"]
            .iter()
            .any(|f| before.record["show"][*f] != after.record["show"][*f])
        || current(&after.face, action_id)? != value
    {
        return Err(refusal(
            "field change lacks exact checked argument/fresh Source and acknowledged Show",
        ));
    }
    Ok(c.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_custody_refuses_stale_or_foreign_receipts() {
        // Receipt-shape test only; production additionally validates Face digest
        // and full manifestation with the existing Look/correspondence decoder.
        let boot = json!({"host_id":"host","boot_id":"boot"});
        let record = json!({"schema":"conduit.native-face/v1","face":{"identity":"face","revision":7},
            "show":{"face_id":"face","face_revision":7,"show_id":"show","host_id":"host","boot_id":"boot","lifecycle":"Available"}});
        let focus = json!({"schema":"conduit.creche-focus/v1","face_id":"face","face_revision":7,"show_id":"show"});
        assert!(focus_custody(&record, &focus, &boot).is_ok());
        for (key, value) in [
            ("face_id", json!("old")),
            ("face_revision", json!(6)),
            ("show_id", json!("old")),
        ] {
            let mut wrong = focus.clone();
            wrong[key] = value;
            assert!(focus_custody(&record, &wrong, &boot).is_err());
        }
        for key in ["host_id", "boot_id", "lifecycle"] {
            let mut wrong = record.clone();
            wrong["show"][key] = json!("foreign");
            assert!(focus_custody(&wrong, &focus, &boot).is_err());
        }
    }
}
