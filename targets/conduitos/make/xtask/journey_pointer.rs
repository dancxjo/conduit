//! Verification of typed pointer routing into retained shell surfaces.

use serde_json::Value;

use super::ConduitosError;

pub(super) struct PointerEvidence {
    pub(super) hover_subject: String,
    pub(super) selected_subject: String,
    pub(super) press_sequence: u64,
    pub(super) release_sequence: u64,
    pub(super) inspector_presentation_id: String,
    pub(super) inspector_manifestation_id: String,
    pub(super) focused_manifestation_id: String,
}

pub(super) fn validate(
    records: &[Value],
    opened: &Value,
) -> Result<PointerEvidence, ConduitosError> {
    if records.len() != 4 {
        return Err(ConduitosError::refusal(
            "product-journey-pointer-record-count",
            "hover, selection, release, and auxiliary focus must produce exactly four records",
        ));
    }
    let hover = &records[0];
    let press = &records[1];
    let release = &records[2];
    let focus = &records[3];
    let press_sequence = number(press, "sequence")?;
    let release_sequence = number(release, "sequence")?;
    if status(hover) != Some("hovered")
        || boolean(hover, "primary_pressed") != Some(false)
        || status(press) != Some("selected")
        || boolean(press, "primary_pressed") != Some(true)
        || status(release) != Some("hovered")
        || boolean(release, "primary_pressed") != Some(false)
        || status(focus) != Some("auxiliary-focused")
        || boolean(focus, "primary_pressed") != Some(true)
        || number(hover, "sequence")? >= press_sequence
        || press_sequence >= release_sequence
        || release_sequence >= number(focus, "sequence")?
    {
        return Err(ConduitosError::refusal(
            "product-journey-pointer-causality-invalid",
            "pointer selection, release, and auxiliary focus were not distinct and ordered",
        ));
    }
    for identity in ["profile_id", "build_id", "image_id", "host_id", "boot_id"] {
        if records
            .iter()
            .any(|record| record.get(identity) != opened.get(identity))
        {
            return Err(ConduitosError::refusal(
                "product-journey-pointer-identity-drift",
                identity,
            ));
        }
    }
    let inspector_presentation_id = text(press, "inspector_presentation_id")?;
    let inspector_manifestation_id = text(press, "inspector_manifestation_id")?;
    let release_manifestation_id = text(release, "inspector_manifestation_id")?;
    let focused_manifestation_id = text(focus, "routed_manifestation_id")?;
    if text(press, "routed_surface_id")? != "conduitos/shell/workspace"
        || text(press, "inspector_surface_id")? != "conduitos/shell/inspector"
        || inspector_presentation_id == text(press, "workspace_presentation_id")?
        || inspector_manifestation_id == text(press, "workspace_manifestation_id")?
        || number(press, "surfaces_composed")? < 3
        || number(press, "damage_count")? == 0
        || text(focus, "routed_surface_id")? != "conduitos/shell/inspector"
        || inspector_manifestation_id == release_manifestation_id
        || focused_manifestation_id != release_manifestation_id
    {
        return Err(ConduitosError::refusal(
            "product-journey-inspector-route-invalid",
            "Gear Back was not independently manifested and focused through its exact route",
        ));
    }
    Ok(PointerEvidence {
        hover_subject: text(hover, "subject")?,
        selected_subject: text(press, "subject")?,
        press_sequence,
        release_sequence,
        inspector_presentation_id,
        inspector_manifestation_id,
        focused_manifestation_id,
    })
}

fn status(record: &Value) -> Option<&str> {
    record.get("status").and_then(Value::as_str)
}

fn boolean(record: &Value, field: &str) -> Option<bool> {
    record.get(field).and_then(Value::as_bool)
}

fn text(record: &Value, field: &str) -> Result<String, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| ConduitosError::refusal("product-journey-pointer-field-missing", field))
}

fn number(record: &Value, field: &str) -> Result<u64, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ConduitosError::refusal("product-journey-pointer-field-missing", field))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn inspector_requires_distinct_identity_revision_and_exact_focus_route() {
        let opened = base("opened", 0, false);
        let mut hover = base("hovered", 1, false);
        hover["subject"] = json!("meet-one-gear/change");
        let mut press = base("selected", 2, true);
        press["subject"] = json!("meet-one-gear/change");
        shell(&mut press, "inspector/1");
        let mut release = base("hovered", 3, false);
        release["subject"] = json!("meet-one-gear/change");
        shell(&mut release, "inspector/2");
        let mut focus = base("auxiliary-focused", 5, true);
        focus["routed_surface_id"] = json!("conduitos/shell/inspector");
        focus["routed_manifestation_id"] = json!("inspector/2");
        let records = vec![hover, press, release, focus];
        validate(&records, &opened).unwrap();

        let mut stale = records;
        stale[3]["routed_manifestation_id"] = json!("inspector/1");
        assert!(validate(&stale, &opened).is_err());
    }

    fn base(status: &str, sequence: u64, pressed: bool) -> Value {
        json!({
            "status": status, "sequence": sequence, "primary_pressed": pressed,
            "profile_id": "profile/1", "build_id": "build/1", "image_id": "image/1",
            "host_id": "host/1", "boot_id": "boot/1"
        })
    }

    fn shell(record: &mut Value, inspector_manifestation: &str) {
        record["routed_surface_id"] = json!("conduitos/shell/workspace");
        record["workspace_presentation_id"] = json!("workspace/presentation");
        record["workspace_manifestation_id"] = json!("workspace/manifestation");
        record["inspector_surface_id"] = json!("conduitos/shell/inspector");
        record["inspector_presentation_id"] = json!("inspector/presentation");
        record["inspector_manifestation_id"] = json!(inspector_manifestation);
        record["surfaces_composed"] = json!(3);
        record["damage_count"] = json!(3);
    }
}
