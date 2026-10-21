//! Copy-failure recovery fixture; CPU faults and semantic Show are separate proof.
use super::super::ConduitosError;
use serde_json::Value;
const PREFIX: &str = "CONDUIT_COMPOSITOR_RECOVERY_PROOF ";
fn expected() -> Value {
    serde_json::json!({
        "schema":"conduit.conduitos/compositor-copy-recovery-fixture@3",
        "status":"completed", "proof_class":"freestanding-emulator", "fixture":true,
        "root_copy_failure":true, "guest_cpu_fault":false, "malformed_guest_reply":false,
        "explicit_fresh_owner":true, "old_staging_released":true, "shared_current_display_grant":true, "already_terminal_staging_released":true,
        "callback_terminal_staging_released":true, "old_clients_alive_at_replacement":true,
        "stale_clients_refused":true, "stale_drop_preserves_replacement":true,
        "semantic_show_acknowledged":false, "physical_scanout":false,
        "fresh_black_pixels":768,
    })
}
pub(super) fn validate(transcript: &str) -> Result<Value, ConduitosError> {
    let refusal = || {
        ConduitosError::refusal(
            "compositor-copy-recovery-fixture-mismatch",
            "requires one exact explicit replacement receipt after a Root copy failure; no guest CPU fault, malformed reply, semantic Show or physical display claim",
        )
    };
    let mut lines = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX));
    let line = lines.next().ok_or_else(refusal)?;
    let value: Value = serde_json::from_str(line).map_err(|_| refusal())?;
    if lines.next().is_some() || value != expected() {
        return Err(refusal());
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_copy_recovery_receipt_preserves_scope_and_counts() {
        let value = expected();
        let line = format!("{PREFIX}{value}\n");
        assert_eq!(validate(&line).unwrap(), value);
        for invalid in [
            String::new(),
            format!("{line}{line}"),
            format!("{PREFIX}{{\n"),
        ] {
            assert!(validate(&invalid).is_err());
        }
        for field in value.as_object().unwrap().keys() {
            let mut changed = value.clone();
            changed[field] = Value::Null;
            assert!(
                validate(&format!("{PREFIX}{changed}\n")).is_err(),
                "{field}"
            );
        }
        let mut extra = value.clone();
        extra["production_acceptance"] = Value::Bool(true);
        assert!(validate(&format!("{PREFIX}{extra}\n")).is_err());
    }
}
