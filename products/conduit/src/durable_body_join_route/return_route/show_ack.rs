//! One acknowledged native Show before each returned semantic action.

use super::*;

const SHOW_ACK_SCHEMA: &str = "conduit.body/native-owner-show-ack@1";
const SHOW_ACK_RESPONSE_SCHEMA: &str = "conduit.body/native-owner-show-ack-response@1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShowAcknowledgement {
    schema: String,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
}

#[derive(Serialize)]
struct ShowAcknowledgementResponse {
    schema: &'static str,
    accepted: bool,
    code: &'static str,
}

pub(super) enum ShowGate {
    Accepted,
    Refreshed(Box<Grant>),
    Stopped,
}

pub(super) fn acknowledge_show(
    listener: &SecureWebSocketListener,
    state_dir: &Path,
    grant: &Grant,
    sequence: u8,
    deadline: Instant,
    allow_refresh: bool,
) -> Result<ShowGate, String> {
    let Some(left) = deadline.checked_duration_since(Instant::now()) else {
        return Ok(ShowGate::Stopped);
    };
    let mut line = match listener.accept_with_timeout(left) {
        Ok(line) => line,
        Err(SecureWebSocketError::AcceptDeadline) => return Ok(ShowGate::Stopped),
        Err(error) => return Err(format!("accept native Show return: {error:?}")),
    };
    line.set_read_timeout(Some(left.min(Duration::from_secs(5))))
        .map_err(|error| format!("bound native Show read: {error:?}"))?;
    let mut frame = vec![0; MAX_OWNER_FACE_RESPONSE_BYTES];
    let mut assembled = ChunkAssembly::default();
    let document: serde_json::Value = match receive_document(&mut line, &mut frame, &mut assembled)
    {
        Ok(document) => document,
        Err(_) => return Ok(ShowGate::Stopped),
    };
    if document.get("schema").and_then(serde_json::Value::as_str) == Some(refresh::REFRESH_SCHEMA) {
        if !allow_refresh {
            return Ok(ShowGate::Stopped);
        }
        return refresh::serve(&mut line, listener, state_dir, grant, sequence, document);
    }
    let outcome = match serde_json::from_value::<ShowAcknowledgement>(document) {
        Ok(ack)
            if ack.schema == SHOW_ACK_SCHEMA
                && ack.sequence == sequence
                && grant.matches_request(ack.token, &ack.request) =>
        {
            line.complete_bounded_input();
            crate::durable_host_control::acknowledge_native_mask_show(
                state_dir,
                ack.request,
                ack.show,
            )
            .map_err(|error| {
                let code =
                    if error.len() <= 128 && error.bytes().all(|byte| (32..=126).contains(&byte)) {
                        error.as_str()
                    } else {
                        "show-refused-detail-invalid"
                    };
                eprintln!(
                    "CONDUIT_OWNER_RETURN_DIAGNOSTIC {}",
                    serde_json::json!({"phase":"show-ack-refused","code":code})
                );
                "show-refused"
            })
        }
        Ok(_) => Err("show-grant-or-basis-invalid"),
        Err(_) => Err("show-frame-invalid"),
    };
    let (accepted, code) = match outcome {
        Ok(()) => (true, "accepted"),
        Err(code) => (false, code),
    };
    send(
        &mut line,
        &ShowAcknowledgementResponse {
            schema: SHOW_ACK_RESPONSE_SCHEMA,
            accepted,
            code,
        },
    )?;
    Ok(if accepted {
        ShowGate::Accepted
    } else {
        ShowGate::Stopped
    })
}
