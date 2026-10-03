//! Short-lived native action return, granted only after one admitted invitation.
//! The grant is a fresh bearer on pinned TLS, not a second use of that invitation.

use super::{current_time_millis, send};
use conduit_body::PortableAdmissionReceipt;
use conduit_presentation::{
    FaceInteraction, MaskShow, OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse,
    MAX_OWNER_FACE_RESPONSE_BYTES,
};
use conduit_std_host::secure_websocket::{SecureWebSocketError, SecureWebSocketListener};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{Duration, Instant},
};

const FACE_REQUEST_SCHEMA: &str = "conduit.body/native-owner-face-request@1";
const GRANT_SCHEMA: &str = "conduit.body/native-owner-return-grant@1";
const ACTION_SCHEMA: &str = "conduit.body/native-owner-return-action@1";
const RESPONSE_SCHEMA: &str = "conduit.body/native-owner-return-response@1";
const MAX_ACTIONS: u8 = 4;
const MAX_LIFETIME_MILLIS: u64 = 60_000;
const MAX_RETURN_ACTION_BYTES: usize = 64 * 1024;
const CHUNK_HEADER_BYTES: usize = 40;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FaceRequest {
    schema: String,
    request: OwnerFaceSnapshotRequest,
}

pub(super) fn decode_face_request(
    bytes: &[u8],
) -> Result<(OwnerFaceSnapshotRequest, bool), String> {
    if let Ok(wrapper) = serde_json::from_slice::<FaceRequest>(bytes) {
        if wrapper.schema != FACE_REQUEST_SCHEMA {
            return Err("unsupported native owner Face request".into());
        }
        return Ok((wrapper.request, true));
    }
    serde_json::from_slice(bytes)
        .map(|request| (request, false))
        .map_err(|error| format!("decode owner Face request: {error}"))
}

#[derive(Serialize)]
pub(super) struct Grant {
    schema: &'static str,
    token: [u8; 32],
    credential_id: String,
    body_id: String,
    part_id: String,
    host_id: String,
    boot_id: String,
    remaining_millis: u32,
    maximum_actions: u8,
    #[serde(skip)]
    expires_at_millis: u64,
}

impl Grant {
    pub(super) fn issue(receipt: &PortableAdmissionReceipt) -> Result<Self, String> {
        let now = current_time_millis()?;
        let remaining = MAX_LIFETIME_MILLIS;
        let mut token = [0; 32];
        getrandom::fill(&mut token).map_err(|error| format!("native return entropy: {error}"))?;
        let credential = &receipt.credential;
        Ok(Self {
            schema: GRANT_SCHEMA,
            token,
            credential_id: credential.credential_id.as_str().into(),
            body_id: credential.body_id.as_str().into(),
            part_id: credential.part_id.as_str().into(),
            host_id: credential.host_id.as_str().into(),
            boot_id: credential.boot_id.as_str().into(),
            remaining_millis: remaining as u32,
            maximum_actions: MAX_ACTIONS,
            expires_at_millis: now + remaining,
        })
    }

    fn basis_matches(&self, action: &Action) -> bool {
        action.schema == ACTION_SCHEMA && self.matches_request(action.token, &action.request)
    }

    fn matches_request(&self, token: [u8; 32], request: &OwnerFaceSnapshotRequest) -> bool {
        let mut difference = 0_u8;
        for (offered, issued) in token.into_iter().zip(self.token) {
            difference |= offered ^ issued;
        }
        difference == 0
            && request.has_exact_basis()
            && request.credential_id == self.credential_id
            && request.body_id.as_str() == self.body_id
            && request.part_id.as_str() == self.part_id
            && request.host_id.as_str() == self.host_id
            && request.boot_id.as_str() == self.boot_id
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    schema: String,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
    interaction: FaceInteraction,
}

#[derive(Serialize)]
struct Response {
    schema: &'static str,
    accepted: bool,
    code: &'static str,
    face: Option<OwnerFaceSnapshotResponse>,
}

pub(super) fn serve(
    listener: &SecureWebSocketListener,
    state_dir: &Path,
    grant: &Grant,
) -> Result<(), String> {
    let remaining = grant
        .expires_at_millis
        .saturating_sub(current_time_millis()?);
    let grant_deadline = Instant::now() + Duration::from_millis(remaining);
    let deadline = grant_deadline;
    for sequence in 1..=grant.maximum_actions {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        if left.is_zero() {
            break;
        }
        if current_time_millis()? >= grant.expires_at_millis {
            break;
        }
        let mut line = match listener.accept_with_timeout(left) {
            Ok(line) => line,
            Err(SecureWebSocketError::AcceptDeadline) => break,
            Err(error) => return Err(format!("accept native return: {error:?}")),
        };
        line.set_read_timeout(Some(left.min(Duration::from_secs(5))))
            .map_err(|error| format!("bound native return read: {error:?}"))?;
        let response = match receive_action(&mut line) {
            Ok(_) if current_time_millis()? >= grant.expires_at_millis => Response {
                schema: RESPONSE_SCHEMA,
                accepted: false,
                code: "return-expired",
                face: None,
            },
            Ok(action) if action.sequence != sequence || !grant.basis_matches(&action) => {
                Response {
                    schema: RESPONSE_SCHEMA,
                    accepted: false,
                    code: "return-grant-or-basis-invalid",
                    face: None,
                }
            }
            Ok(action) => {
                line.complete_bounded_input();
                apply(
                    state_dir,
                    action,
                    sequence < grant.maximum_actions,
                    grant.expires_at_millis,
                )
            }
            Err(_) => Response {
                schema: RESPONSE_SCHEMA,
                accepted: false,
                code: "return-frame-invalid",
                face: None,
            },
        };
        let outcome_unknown = response.code == "control-outcome-unknown";
        send(&mut line, &fit_response(response)?)?;
        if outcome_unknown {
            break;
        }
    }
    Ok(())
}

fn receive_action(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
) -> Result<Action, String> {
    let mut frame = vec![0; MAX_OWNER_FACE_RESPONSE_BYTES];
    let mut assembled = ChunkAssembly::default();
    // Each frame carries the same finite total and digest plus its exact offset.
    // The WebSocket Line provides ordering and pinned TLS; these checks keep
    // malformed or interrupted multi-frame actions distinct from submission.
    for _ in 0..=MAX_RETURN_ACTION_BYTES / (MAX_OWNER_FACE_RESPONSE_BYTES - CHUNK_HEADER_BYTES) {
        let length = line
            .receive_binary(&mut frame)
            .map_err(|error| format!("receive native return: {error:?}"))?;
        if assembled.push(&frame[..length])? {
            let action = serde_json::from_slice(&assembled.bytes)
                .map_err(|error| format!("decode native return: {error}"))?;
            return Ok(action);
        }
    }
    Err("native return chunk count exceeded".into())
}

#[derive(Default)]
struct ChunkAssembly {
    bytes: Vec<u8>,
    expected: Option<(usize, [u8; 32])>,
}

impl ChunkAssembly {
    fn push(&mut self, frame: &[u8]) -> Result<bool, String> {
        if frame.len() <= CHUNK_HEADER_BYTES || frame.len() > MAX_OWNER_FACE_RESPONSE_BYTES {
            return Err("native return chunk size invalid".into());
        }
        let total = u32::from_le_bytes(frame[..4].try_into().expect("four header bytes")) as usize;
        let offset =
            u32::from_le_bytes(frame[4..8].try_into().expect("four header bytes")) as usize;
        let digest: [u8; 32] = frame[8..CHUNK_HEADER_BYTES]
            .try_into()
            .expect("digest header");
        if total == 0 || total > MAX_RETURN_ACTION_BYTES || offset != self.bytes.len() {
            return Err("native return chunk bound or offset invalid".into());
        }
        if let Some((prior_total, prior_digest)) = self.expected {
            if (total, digest) != (prior_total, prior_digest) {
                return Err("native return chunk identity changed".into());
            }
        } else {
            self.bytes.reserve_exact(total);
            self.expected = Some((total, digest));
        }
        if frame.len() - CHUNK_HEADER_BYTES > total - offset {
            return Err("native return chunk exceeds total".into());
        }
        self.bytes.extend_from_slice(&frame[CHUNK_HEADER_BYTES..]);
        if self.bytes.len() != total {
            return Ok(false);
        }
        if Sha256::digest(&self.bytes).as_slice() != digest {
            return Err("native return chunk digest mismatch".into());
        }
        Ok(true)
    }
}

fn fit_response(mut response: Response) -> Result<Response, String> {
    let bytes = serde_json::to_vec(&response)
        .map_err(|error| format!("encode native return response: {error}"))?;
    if bytes.len() > MAX_OWNER_FACE_RESPONSE_BYTES {
        response.face = None;
        if response.accepted {
            response.code = "accepted-face-pressure";
        }
    }
    Ok(response)
}

fn apply(
    state_dir: &Path,
    action: Action,
    more_actions: bool,
    grant_expires_at_millis: u64,
) -> Response {
    let request = action.request;
    let result = crate::durable_host_control::submit_browser_face_interaction(
        state_dir,
        request.clone(),
        action.show,
        action.interaction,
    );
    let (accepted, code) = match &result {
        Ok(_) => (true, "accepted"),
        Err(error) if error == "control-outcome-unknown" => (false, "control-outcome-unknown"),
        Err(error)
            if error.contains("StaleFace")
                || error.contains("StaleShow")
                || error.contains("stale owner Mask Show") =>
        {
            (false, "stale-face-or-show")
        }
        Err(error)
            if error == "body-play-active"
                || error.contains("retired Play")
                || error.contains("clock-play-must-lull") =>
        {
            (false, "clock-play-must-lull")
        }
        Err(error) if error.contains("UnavailableAction") => (false, "action-unavailable"),
        Err(error) if error.contains("RefusedAction") => (false, "action-refused"),
        Err(error) if error.contains("credential-not-admitted") => {
            (false, "credential-not-admitted")
        }
        Err(error) if error.contains("current-part-unavailable") => {
            (false, "current-part-unavailable")
        }
        Err(_) => (false, "interaction-refused"),
    };
    let face = crate::durable_host_control::face_snapshot(state_dir, request)
        .ok()
        .map(|mut response| {
            if let OwnerFaceSnapshotResponse::Snapshot {
                interactions_admitted,
                ..
            } = &mut response
            {
                *interactions_admitted = more_actions
                    && code != "control-outcome-unknown"
                    && current_time_millis().is_ok_and(|now| now < grant_expires_at_millis);
            }
            response
        });
    Response {
        schema: RESPONSE_SCHEMA,
        accepted,
        code,
        face,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::OWNER_FACE_REQUEST_SCHEMA;

    fn request() -> OwnerFaceSnapshotRequest {
        OwnerFaceSnapshotRequest {
            schema: OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: "credential/guest".into(),
            body_id: serde_json::from_str("\"body/one\"").unwrap(),
            part_id: serde_json::from_str("\"part/one\"").unwrap(),
            host_id: "host/guest".into(),
            boot_id: "boot/guest".into(),
            last_seen_revision: None,
            last_seen_identity: None,
        }
    }

    fn chunks(payload: &[u8]) -> Vec<Vec<u8>> {
        let digest = Sha256::digest(payload);
        payload
            .chunks(MAX_OWNER_FACE_RESPONSE_BYTES - CHUNK_HEADER_BYTES)
            .enumerate()
            .map(|(index, chunk)| {
                let mut frame = Vec::with_capacity(CHUNK_HEADER_BYTES + chunk.len());
                frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                frame.extend_from_slice(
                    &((index * (MAX_OWNER_FACE_RESPONSE_BYTES - CHUNK_HEADER_BYTES)) as u32)
                        .to_le_bytes(),
                );
                frame.extend_from_slice(&digest);
                frame.extend_from_slice(chunk);
                frame
            })
            .collect()
    }

    #[test]
    fn chunk_assembly_accepts_large_exact_payload_and_refuses_offset_digest_and_pressure() {
        let payload = vec![0x5a; 20_000];
        let frames = chunks(&payload);
        assert_eq!(frames.len(), 3);
        let mut assembly = ChunkAssembly::default();
        assert!(!assembly.push(&frames[0]).unwrap());
        assert!(!assembly.push(&frames[1]).unwrap());
        assert!(assembly.push(&frames[2]).unwrap());
        assert_eq!(assembly.bytes, payload);

        let mut reordered = ChunkAssembly::default();
        assert!(reordered.push(&frames[1]).is_err());
        let mut duplicate = ChunkAssembly::default();
        assert!(!duplicate.push(&frames[0]).unwrap());
        assert!(duplicate.push(&frames[0]).is_err());
        let mut switched = frames.clone();
        switched[1][8] ^= 1;
        let mut assembly = ChunkAssembly::default();
        assert!(!assembly.push(&switched[0]).unwrap());
        assert!(assembly.push(&switched[1]).is_err());
        let mut corrupt = frames.clone();
        corrupt[2][CHUNK_HEADER_BYTES] ^= 1;
        let mut assembly = ChunkAssembly::default();
        assert!(!assembly.push(&corrupt[0]).unwrap());
        assert!(!assembly.push(&corrupt[1]).unwrap());
        assert!(assembly.push(&corrupt[2]).is_err());
        let mut oversized = frames[0].clone();
        oversized[..4].copy_from_slice(&((MAX_RETURN_ACTION_BYTES + 1) as u32).to_le_bytes());
        assert!(ChunkAssembly::default().push(&oversized).is_err());
        assert!(ChunkAssembly::default()
            .push(&[0; CHUNK_HEADER_BYTES])
            .is_err());
    }

    #[test]
    fn only_explicit_native_face_request_opts_into_return() {
        let plain = serde_json::to_vec(&request()).unwrap();
        assert!(!decode_face_request(&plain).unwrap().1);
        let wrapped = serde_json::to_vec(&serde_json::json!({
            "schema":FACE_REQUEST_SCHEMA, "request":request(),
        }))
        .unwrap();
        assert!(decode_face_request(&wrapped).unwrap().1);
        let wrong = serde_json::to_vec(&serde_json::json!({
            "schema":"conduit.body/unknown", "request":request(),
        }))
        .unwrap();
        assert!(decode_face_request(&wrong).is_err());
    }

    #[test]
    fn grant_is_bearer_bound_to_exact_credential_and_current_boot_claim() {
        let grant = Grant {
            schema: GRANT_SCHEMA,
            token: [7; 32],
            credential_id: "credential/guest".into(),
            body_id: "body/one".into(),
            part_id: "part/one".into(),
            host_id: "host/guest".into(),
            boot_id: "boot/guest".into(),
            remaining_millis: 60_000,
            maximum_actions: MAX_ACTIONS,
            expires_at_millis: 60_000,
        };
        assert!(grant.matches_request([7; 32], &request()));
        assert!(!grant.matches_request([8; 32], &request()));
        let mut stale = request();
        stale.boot_id = "boot/old".into();
        assert!(!grant.matches_request([7; 32], &stale));
        stale = request();
        stale.part_id = serde_json::from_str("\"part/other\"").unwrap();
        assert!(!grant.matches_request([7; 32], &stale));
    }

    #[test]
    fn oversized_refreshed_face_keeps_action_truth_and_refuses_guest_frame_pressure() {
        let response = Response {
            schema: RESPONSE_SCHEMA,
            accepted: true,
            code: "accepted",
            face: Some(OwnerFaceSnapshotResponse::Refused {
                schema: "conduit.presentation/owner-face-response@1".into(),
                code: "x".repeat(MAX_OWNER_FACE_RESPONSE_BYTES),
            }),
        };
        let fitted = fit_response(response).unwrap();
        assert!(fitted.accepted);
        assert_eq!(fitted.code, "accepted-face-pressure");
        assert!(fitted.face.is_none());
        assert!(serde_json::to_vec(&fitted).unwrap().len() <= MAX_OWNER_FACE_RESPONSE_BYTES);
    }
}
