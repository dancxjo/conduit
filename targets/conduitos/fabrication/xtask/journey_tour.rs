//! Verification of the canonical Tour and its retained shell manifestations.

use std::collections::BTreeMap;

use serde_json::Value;

use super::ConduitosError;

pub(super) fn validate(records: &[Value], opened: &Value) -> Result<(), ConduitosError> {
    let by_status = records
        .iter()
        .filter_map(|record| Some((record.get("status")?.as_str()?.to_owned(), record)))
        .collect::<BTreeMap<_, _>>();
    for status in ["tour-opened", "result-visible", "patchbay-open"] {
        if !by_status.contains_key(status) {
            return Err(ConduitosError::refusal(
                "product-journey-tour-stage-missing",
                status,
            ));
        }
    }
    let tour_opened = by_status["tour-opened"];
    let tour_result = by_status["result-visible"];
    let tour_patchbay = by_status["patchbay-open"];
    if text(tour_opened, "specimen_id")? != "canonical-form:meet-one-gear"
        || tour_opened.get("plan_id") != Some(&Value::Null)
        || text(tour_result, "result")? != "HELLO"
        || tour_result.get("plan_id") == Some(&Value::Null)
        || tour_result.get("active_play_id") == Some(&Value::Null)
        || tour_patchbay.get("result") != tour_result.get("result")
        || tour_patchbay.get("plan_id") != Some(&Value::Null)
    {
        return Err(ConduitosError::refusal(
            "product-journey-tour-causality-invalid",
            "Tour open, production Play, result, and Patchbay continuity did not match",
        ));
    }
    for identity in ["profile_id", "build_id", "image_id", "host_id", "boot_id"] {
        let expected = opened.get(identity);
        if expected.is_none()
            || records
                .iter()
                .any(|record| record.get(identity) != expected)
        {
            return Err(ConduitosError::refusal(
                "product-journey-tour-identity-drift",
                identity,
            ));
        }
    }
    for record in records {
        validate_shell(record)?;
    }
    Ok(())
}

fn validate_shell(record: &Value) -> Result<(), ConduitosError> {
    let workspace_surface = text(record, "workspace_surface_id")?;
    let status_surface = text(record, "status_surface_id")?;
    let workspace_presentation = text(record, "workspace_presentation_id")?;
    let status_presentation = text(record, "status_presentation_id")?;
    let workspace_manifestation = text(record, "workspace_manifestation_id")?;
    let status_manifestation = text(record, "status_manifestation_id")?;
    if workspace_surface != "conduitos/shell/workspace"
        || status_surface != "conduitos/shell/status"
        || workspace_surface == status_surface
        || workspace_presentation == status_presentation
        || workspace_manifestation == status_manifestation
        || number(record, "surfaces_composed")? < 2
        || number(record, "damage_count")? == 0
    {
        return Err(ConduitosError::refusal(
            "product-journey-tour-shell-invalid",
            "workspace and status were not distinct simultaneous retained manifestations",
        ));
    }
    Ok(())
}

fn text(record: &Value, field: &str) -> Result<String, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| ConduitosError::refusal("product-journey-tour-shell-field-missing", field))
}

fn number(record: &Value, field: &str) -> Result<u64, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ConduitosError::refusal("product-journey-tour-shell-field-missing", field))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn distinct_simultaneous_shell_manifestations_are_required() {
        let opened = identity_record("form-opened");
        let mut tour_opened = shell_record("tour-opened");
        tour_opened["specimen_id"] = json!("canonical-form:meet-one-gear");
        tour_opened["plan_id"] = Value::Null;
        let mut result = shell_record("result-visible");
        result["result"] = json!("HELLO");
        result["plan_id"] = json!("plan/1");
        result["active_play_id"] = json!("play/1");
        let mut patchbay = shell_record("patchbay-open");
        patchbay["result"] = json!("HELLO");
        patchbay["plan_id"] = Value::Null;
        let records = vec![tour_opened, result, patchbay];
        validate(&records, &opened).unwrap();

        let mut aliased = records;
        aliased[0]["status_manifestation_id"] = json!("manifestation/workspace");
        assert!(validate(&aliased, &opened).is_err());
    }

    fn identity_record(status: &str) -> Value {
        json!({
            "status": status,
            "profile_id": "profile/1",
            "build_id": "build/1",
            "image_id": "image/1",
            "host_id": "host/1",
            "boot_id": "boot/1"
        })
    }

    fn shell_record(status: &str) -> Value {
        let mut record = identity_record(status);
        record["workspace_surface_id"] = json!("conduitos/shell/workspace");
        record["workspace_presentation_id"] = json!("presentation/workspace");
        record["workspace_manifestation_id"] = json!("manifestation/workspace");
        record["status_surface_id"] = json!("conduitos/shell/status");
        record["status_presentation_id"] = json!("presentation/status");
        record["status_manifestation_id"] = json!("manifestation/status");
        record["surfaces_composed"] = json!(2);
        record["damage_count"] = json!(2);
        record
    }
}
