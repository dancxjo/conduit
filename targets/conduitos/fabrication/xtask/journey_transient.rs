//! Verification of live transient Presentation surfaces and dismissal.

use serde_json::Value;

use super::ConduitosError;

pub(super) struct TransientEvidence {
    pub(super) kinds: Vec<String>,
    pub(super) refusal_cause: String,
    pub(super) chooser_manifestation_id: String,
    pub(super) stale_input_refused: bool,
}

pub(super) fn validate(
    records: &[Value],
    pointer_records: &[Value],
    opened: &Value,
) -> Result<TransientEvidence, ConduitosError> {
    if records.len() != 6 {
        return Err(refusal(
            "product-journey-transient-record-count",
            "confirmation, refusal, and chooser must each be shown and dismissed",
        ));
    }
    let expected = ["confirmation", "refusal", "chooser"];
    for (index, kind) in expected.into_iter().enumerate() {
        let shown = &records[index * 2];
        let dismissed = &records[index * 2 + 1];
        if text(shown, "status")? != "shown"
            || text(shown, "kind")? != kind
            || text(dismissed, "status")? != "dismissed"
            || text(shown, "surface_id")? != "conduitos/shell/transient"
            || shown.get("presentation_id") == shown.get("parent_presentation_id")
            || shown.get("manifestation_id") == shown.get("parent_manifestation_id")
            || shown.get("surface_id") != shown.get("keyboard_surface_id")
            || shown.get("manifestation_id") != shown.get("keyboard_manifestation_id")
            || shown.get("manifestation_id") != dismissed.get("manifestation_id")
            || number(shown, "damage_count")? == 0
            || number(dismissed, "damage_count")? == 0
            || number(shown, "pixels_written")? == 0
            || number(dismissed, "pixels_written")? == 0
            || number(shown, "frame_sequence")? >= number(dismissed, "frame_sequence")?
        {
            return Err(refusal("product-journey-transient-lifecycle-invalid", kind));
        }
        for identity in ["profile_id", "build_id", "image_id", "host_id", "boot_id"] {
            if shown.get(identity) != opened.get(identity)
                || dismissed.get(identity) != opened.get(identity)
            {
                return Err(refusal(
                    "product-journey-transient-identity-drift",
                    identity,
                ));
            }
        }
    }
    let refusal_record = &records[2];
    let cause = text(refusal_record, "cause")?;
    if cause != "unknown-action" {
        return Err(refusal(
            "product-journey-transient-refusal-cause-invalid",
            cause,
        ));
    }
    let chooser = &records[4];
    let chooser_dismissed = &records[5];
    let chooser_manifestation_id = text(chooser, "manifestation_id")?;
    let pointer = pointer_records
        .iter()
        .find(|record| record.get("status").and_then(Value::as_str) == Some("transient-focused"))
        .ok_or_else(|| refusal("product-journey-transient-pointer-route-missing", "chooser"))?;
    if text(pointer, "routed_surface_id")? != "conduitos/shell/transient"
        || text(pointer, "routed_manifestation_id")? != chooser_manifestation_id
        || chooser_dismissed
            .get("stale_input_refused")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(refusal(
            "product-journey-transient-pointer-route-invalid",
            "chooser route or stale dismissal did not match",
        ));
    }
    Ok(TransientEvidence {
        kinds: expected.iter().map(|kind| (*kind).into()).collect(),
        refusal_cause: cause,
        chooser_manifestation_id,
        stale_input_refused: true,
    })
}

fn text(record: &Value, field: &str) -> Result<String, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| refusal("product-journey-transient-field-missing", field))
}

fn number(record: &Value, field: &str) -> Result<u64, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("product-journey-transient-field-missing", field))
}

fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn requires_three_related_transients_and_exact_stale_chooser_route() {
        let opened = base_context();
        let mut records = Vec::new();
        for (index, kind) in ["confirmation", "refusal", "chooser"].iter().enumerate() {
            records.push(shown(kind, index as u64 * 2 + 1));
            records.push(dismissed(index as u64 * 2 + 2, kind == &"chooser"));
        }
        let pointer = vec![json!({
            "status":"transient-focused",
            "routed_surface_id":"conduitos/shell/transient",
            "routed_manifestation_id":"manifestation/chooser"
        })];
        validate(&records, &pointer, &opened).unwrap();
        records[5]["stale_input_refused"] = json!(false);
        assert!(validate(&records, &pointer, &opened).is_err());
    }

    fn base_context() -> Value {
        json!({"profile_id":"p","build_id":"b","image_id":"i","host_id":"h","boot_id":"boot"})
    }

    fn shown(kind: &str, frame: u64) -> Value {
        json!({
            "status":"shown", "kind":kind,
            "cause": if kind == "refusal" { Value::String("unknown-action".into()) } else { Value::Null },
            "surface_id":"conduitos/shell/transient",
            "presentation_id":format!("presentation/{kind}"),
            "manifestation_id":format!("manifestation/{kind}"),
            "parent_presentation_id":"presentation/workspace",
            "parent_manifestation_id":"manifestation/workspace",
            "keyboard_surface_id":"conduitos/shell/transient",
            "keyboard_manifestation_id":format!("manifestation/{kind}"),
            "frame_sequence":frame, "damage_count":1, "pixels_written":1,
            "profile_id":"p","build_id":"b","image_id":"i","host_id":"h","boot_id":"boot"
        })
    }

    fn dismissed(frame: u64, stale: bool) -> Value {
        let kind = match frame {
            2 => "confirmation",
            4 => "refusal",
            _ => "chooser",
        };
        json!({
            "status":"dismissed", "surface_id":"conduitos/shell/transient",
            "manifestation_id":format!("manifestation/{kind}"),
            "frame_sequence":frame, "damage_count":1, "pixels_written":1,
            "stale_input_refused":stale,
            "profile_id":"p","build_id":"b","image_id":"i","host_id":"h","boot_id":"boot"
        })
    }
}
