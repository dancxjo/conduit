//! Exact semantic copy-recovery receipt, preserving the atomic present boundary.
use super::super::ConduitosError;
use serde_json::Value;
const PREFIX: &str = "CONDUIT_COMPOSITOR_SEMANTIC_RECOVERY_PROOF ";
fn expected() -> Value {
    serde_json::json!({"schema":"conduit.conduitos/compositor-semantic-copy-recovery-fixture@1",
        "status":"completed","proof_class":"freestanding-emulator","fixture":true,
        "production_mask_proxy":true,"initial_show_acknowledged":true,
        "root_copy_failure":true,"old_submit_refused":true,
        "stale_input_preserves_semantic_revision":true,"fresh_service_instance":true,
        "no_show_or_keyboard_before_present":true,"atomic_present_acknowledgement":true,
        "fresh_show_and_play":true,"held_input_refused_by_fresh_show":true,
        "fresh_interaction_correlated":true,"fresh_interaction_advances_semantics":true,
        "physical_scanout":false,"human_observation":false,
        "guest_cpu_fault_recovery":false,"malformed_guest_reply_recovery":false})
}
pub(super) fn validate(transcript: &str) -> Result<Value, ConduitosError> {
    let refusal = || {
        ConduitosError::refusal(
            "compositor-semantic-copy-recovery-mismatch",
            "requires one exact production Mask/proxy copy-failure recovery fixture; public present is atomic, with no physical/human, guest CPU-fault or malformed-reply semantic recovery claim",
        )
    };
    let mut lines = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX));
    let value: Value =
        serde_json::from_str(lines.next().ok_or_else(refusal)?).map_err(|_| refusal())?;
    if lines.next().is_some() || value != expected() {
        return Err(refusal());
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_atomic_semantic_recovery_and_nonclaims_are_required() {
        let value = expected();
        let line = format!("{PREFIX}{value}\n");
        assert_eq!(validate(&line).unwrap(), value);
        for bad in [
            String::new(),
            format!("{line}{line}"),
            format!("{PREFIX}{{\n"),
        ] {
            assert!(validate(&bad).is_err());
        }
        for field in value.as_object().unwrap().keys() {
            let mut bad = value.clone();
            bad[field] = Value::Null;
            assert!(
                validate(&format!("{PREFIX}{bad}\n")).is_err(),
                "missing {field}"
            );
            if let Some(flag) = value[field].as_bool() {
                bad[field] = Value::Bool(!flag);
                assert!(
                    validate(&format!("{PREFIX}{bad}\n")).is_err(),
                    "flipped {field}"
                );
            }
        }
        let mut extra = value.clone();
        extra["direct_memory_isolation"] = Value::Bool(true);
        assert!(validate(&format!("{PREFIX}{extra}\n")).is_err());
    }
}
