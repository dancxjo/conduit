//! Verify the one explicitly held screen-free Start between guest standby and F5.
use serde_json::Value;

use super::{refusal, ConduitosError};

pub(super) fn validate_held_start(
    basis: &Value,
    guest_part: &Value,
    standby_face: &Value,
    current_face: &Value,
    source_commit: &str,
) -> Result<(), ConduitosError> {
    let source_revision = basis
        .get("source_face_revision")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok());
    let result_revision = basis
        .get("result_face_revision")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok());
    let valid = basis.get("schema").and_then(Value::as_str)
        == Some("conduit.proof/held-clock-start@1")
        && basis.get("source_commit").and_then(Value::as_str) == Some(source_commit)
        && basis.get("body_id") == guest_part.get("body_id")
        && basis.get("guest_part_id") == guest_part.get("part_id")
        && basis.get("guest_boot_id") == guest_part.get("boot_id")
        && basis.get("standby_face_id") == standby_face.get("face_id")
        && basis.get("standby_face_revision") == standby_face.get("face_revision")
        && basis.get("source_face_id") == standby_face.get("face_id")
        && source_revision == standby_face.get("face_revision").and_then(Value::as_u64)
        && basis.get("result_face_id") == current_face.get("face_id")
        && result_revision == current_face.get("face_revision").and_then(Value::as_u64)
        && matches!((source_revision, result_revision), (Some(before), Some(after)) if after > before)
        && basis
            .get("action_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("body/action/start-clock/"))
        && basis
            .get("transcript_sha256")
            .and_then(Value::as_str)
            .is_some_and(|hash| {
                hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        && basis
            .get("observed_at_unix_ms")
            .and_then(Value::as_u64)
            .is_some_and(|time| time > 0);
    if !valid {
        return Err(refusal("native-owner-held-start-invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn held_start_permits_only_the_recorded_face_advance_for_the_same_guest() {
        let part = json!({"body_id":"body/one","part_id":"part/guest","boot_id":"boot/guest"});
        let standby = json!({"face_id":"face/before","face_revision":7});
        let active = json!({"face_id":"face/after","face_revision":8});
        let basis = json!({
            "schema":"conduit.proof/held-clock-start@1",
            "source_commit":"a".repeat(40),
            "body_id":"body/one", "guest_part_id":"part/guest", "guest_boot_id":"boot/guest",
            "standby_face_id":"face/before", "standby_face_revision":7,
            "source_face_id":"face/before", "source_face_revision":"7",
            "result_face_id":"face/after", "result_face_revision":"8",
            "action_id":"body/action/start-clock/0", "transcript_sha256":"b".repeat(64),
            "observed_at_unix_ms":1,
        });
        assert!(validate_held_start(&basis, &part, &standby, &active, &"a".repeat(40)).is_ok());
        let mut stale = basis.clone();
        stale["result_face_revision"] = json!("7");
        assert!(validate_held_start(&stale, &part, &standby, &active, &"a".repeat(40)).is_err());
        let mut foreign = basis;
        foreign["guest_part_id"] = json!("part/other");
        assert!(validate_held_start(&foreign, &part, &standby, &active, &"a".repeat(40)).is_err());
    }
}
