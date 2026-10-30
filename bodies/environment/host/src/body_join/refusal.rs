//! Bounded nonsecret refusal diagnostics shared by membership clients.
use super::*;
pub(crate) fn reason(bytes: &[u8]) -> String {
    #[derive(Deserialize)]
    struct Refusal {
        schema: String,
        reason: Option<String>,
    }
    let Ok(reply) = serde_json::from_slice::<Refusal>(bytes) else {
        return "unspecified".into();
    };
    match reply.reason {
        Some(reason)
            if reply.schema == "conduit.environment/body-join-refused@1"
                && !reason.is_empty()
                && reason.len() <= 64
                && reason
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') =>
        {
            reason
        }
        _ => "unspecified".into(),
    }
}
