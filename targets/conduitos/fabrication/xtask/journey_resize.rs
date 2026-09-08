//! Verification of the live auxiliary surface resize and relayout lifecycle.

use serde_json::Value;

use super::ConduitosError;

pub(super) struct ResizeEvidence {
    pub(super) surface_id: String,
    pub(super) invalidated_manifestation_id: String,
    pub(super) current_manifestation_id: String,
    pub(super) input_refused_while_invalidated: bool,
}

pub(super) fn validate(
    records: &[Value],
    pointer_records: &[Value],
    opened: &Value,
) -> Result<ResizeEvidence, ConduitosError> {
    let [record] = records else {
        return Err(refusal(
            "product-journey-resize-record-count",
            "exactly one live inspector relayout is required",
        ));
    };
    let surface_id = text(record, "surface_id")?;
    let invalidated_manifestation_id = text(record, "invalidated_manifestation_id")?;
    let current_manifestation_id = text(record, "current_manifestation_id")?;
    if text(record, "status")? != "current"
        || surface_id != "conduitos/shell/inspector"
        || invalidated_manifestation_id == current_manifestation_id
        || number(record, "previous_width")? <= number(record, "current_width")?
        || number(record, "previous_height")? != number(record, "current_height")?
        || record
            .get("input_refused_while_invalidated")
            .and_then(Value::as_bool)
            != Some(true)
        || number(record, "damage_count")? == 0
        || number(record, "pixels_written")? == 0
    {
        return Err(refusal(
            "product-journey-resize-lifecycle-invalid",
            "inspector was not invalidated, relaid out, and repainted distinctly",
        ));
    }
    for identity in ["profile_id", "build_id", "image_id", "host_id", "boot_id"] {
        if record.get(identity) != opened.get(identity) {
            return Err(refusal("product-journey-resize-identity-drift", identity));
        }
    }
    let focused = pointer_records
        .iter()
        .find(|record| record.get("status").and_then(Value::as_str) == Some("auxiliary-focused"))
        .ok_or_else(|| refusal("product-journey-resize-focus-missing", "inspector"))?;
    if focused.get("routed_surface_id") != record.get("surface_id")
        || focused.get("routed_manifestation_id") != record.get("current_manifestation_id")
    {
        return Err(refusal(
            "product-journey-resize-focus-stale",
            "post-resize focus did not use the current geometry binding",
        ));
    }
    Ok(ResizeEvidence {
        surface_id,
        invalidated_manifestation_id,
        current_manifestation_id,
        input_refused_while_invalidated: true,
    })
}

fn text(record: &Value, field: &str) -> Result<String, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| refusal("product-journey-resize-field-missing", field))
}

fn number(record: &Value, field: &str) -> Result<u64, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("product-journey-resize-field-missing", field))
}

fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn requires_fresh_relayout_and_exact_post_resize_focus() {
        let opened =
            json!({"profile_id":"p","build_id":"b","image_id":"i","host_id":"h","boot_id":"boot"});
        let resize = json!({
            "status":"current", "surface_id":"conduitos/shell/inspector",
            "invalidated_manifestation_id":"old", "current_manifestation_id":"new",
            "previous_width":200, "current_width":160, "previous_height":300, "current_height":300,
            "input_refused_while_invalidated":true, "damage_count":2, "pixels_written":600,
            "profile_id":"p","build_id":"b","image_id":"i","host_id":"h","boot_id":"boot"
        });
        let focus = json!({"status":"auxiliary-focused","routed_surface_id":"conduitos/shell/inspector","routed_manifestation_id":"new"});
        validate(
            core::slice::from_ref(&resize),
            core::slice::from_ref(&focus),
            &opened,
        )
        .unwrap();
        let mut stale = focus;
        stale["routed_manifestation_id"] = json!("old");
        assert!(validate(&[resize], &[stale], &opened).is_err());
    }
}
