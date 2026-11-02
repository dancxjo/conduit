//! Explicit authenticated departure; expiry alone never implies Host loss.
use super::*;
pub(super) const SCHEMA: &str = "conduit.body/native-owner-member-leave@1";
const RESPONSE: &str = "conduit.body/native-owner-member-leave-response@1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Leave {
    schema: String,
    token: [u8; 32],
    request: OwnerFaceSnapshotRequest,
}
pub(super) fn requested(document: &serde_json::Value) -> bool {
    document.get("schema").and_then(serde_json::Value::as_str) == Some(SCHEMA)
}
pub(super) fn serve(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    state_dir: &Path,
    grant: &Grant,
    document: serde_json::Value,
) -> Result<(), String> {
    let permitted = authorize_document(grant, document, current_time_millis()?).is_ok();
    let outcome = if permitted {
        line.complete_bounded_input();
        crate::durable_host_control::leave_member(state_dir, grant.member.credential.clone())
    } else {
        Err("member-leave-grant-or-basis-invalid".into())
    };
    let code = match &outcome {
        Ok(()) => "accepted",
        Err(error) if error == "control-outcome-unknown" => "control-outcome-unknown",
        Err(_) => "member-leave-refused",
    };
    send(
        line,
        &serde_json::json!({"schema": RESPONSE, "accepted":outcome.is_ok(), "code":code}),
    )
}

pub(super) fn authorize_document(
    grant: &Grant,
    document: serde_json::Value,
    now: u64,
) -> Result<(), &'static str> {
    let leave: Leave =
        serde_json::from_value(document).map_err(|_| "member-leave-frame-invalid")?;
    if leave.schema != SCHEMA || !grant.matches_request(leave.token, &leave.request) {
        return Err("member-leave-grant-or-basis-invalid");
    }
    if now >= grant.expires_at_millis {
        return Err("member-leave-expired");
    }
    Ok(())
}
