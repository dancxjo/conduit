//! Short-lived native action return, granted only after one admitted invitation.
//! The grant is a fresh bearer on pinned TLS, not a second use of that invitation.

use super::{current_time_millis, send};
#[path = "return_route/action.rs"]
mod action;
#[path = "return_route/component_selection.rs"]
mod component_selection;
#[path = "return_route/face_request.rs"]
mod face_request;
#[path = "return_route/refresh.rs"]
mod refresh;
#[path = "return_route/show_ack.rs"]
mod show_ack;
use conduit_body::PortableAdmissionReceipt;
use conduit_face::{
    FaceInteraction, MaskShow, OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse,
    MAX_OWNER_FACE_RESPONSE_BYTES,
};
use conduit_std_host::secure_websocket::{SecureWebSocketError, SecureWebSocketListener};
pub(super) use face_request::{decode_face_request, MaskSelection};
#[path = "return_route/mask_selection.rs"]
mod mask_selection;
#[cfg(test)]
use face_request::FACE_REQUEST_SCHEMA;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{Duration, Instant},
};

const GRANT_SCHEMA: &str = "conduit.body/native-owner-return-grant@1";
const ACTION_SCHEMA: &str = "conduit.body/native-owner-return-action@1";
const RESPONSE_SCHEMA: &str = "conduit.body/native-owner-return-response@1";
const MAX_ACTIONS: u8 = 4;
const MAX_FACE_REFRESHES: u8 = 4;
const MAX_LIFETIME_MILLIS: u64 = 60_000;
const MAX_RETURN_ACTION_BYTES: usize = 64 * 1024;
const CHUNK_HEADER_BYTES: usize = 40;
// ConduitOS emits frames of at most 8 KiB; a 64 KiB Show or action may
// therefore require nine chunks. Keep a finite margin for smaller frames.
const MAX_RETURN_CHUNKS: usize = 16;

#[derive(Clone, Serialize)]
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
    #[serde(skip)]
    receipt: PortableAdmissionReceipt,
    #[serde(skip)]
    mask: MaskSelection,
}

impl Grant {
    pub(super) fn issue(
        receipt: &PortableAdmissionReceipt,
        mask: MaskSelection,
    ) -> Result<Self, String> {
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
            receipt: receipt.clone(),
            mask,
        })
    }

    pub(super) fn binding_reference(&self, sequence: u8) -> String {
        let digest = Sha256::digest(self.token);
        let digest: String = digest[..12]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{digest}/{sequence}")
    }

    fn basis_matches(&self, action: &Action) -> bool {
        action.schema == ACTION_SCHEMA && self.matches_request(action.token, &action.request)
    }

    fn replacement(&self, next_sequence: u8) -> Result<Self, String> {
        let now = current_time_millis()?;
        if now >= self.expires_at_millis
            || next_sequence == 0
            || next_sequence > self.maximum_actions.saturating_add(1)
        {
            return Err("native return replacement expired or sequence invalid".into());
        }
        let mut grant = Self::issue(&self.receipt, self.mask)?;
        grant.expires_at_millis = self.expires_at_millis;
        grant.remaining_millis = self
            .expires_at_millis
            .saturating_sub(now)
            .min(u64::from(u32::MAX)) as u32;
        grant.maximum_actions = self.maximum_actions.saturating_sub(next_sequence - 1);
        Ok(grant)
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
    let mut grant = grant.clone();
    let mut refreshes = 0_u8;
    // Prepare the complete finite reassembly envelope before any return
    // action is accepted. Reuse it for every admitted action on this route.
    let mut frame = vec![0; MAX_OWNER_FACE_RESPONSE_BYTES];
    let mut assembled = ChunkAssembly::default();
    let mut sequence = 1_u8;
    while sequence <= grant.maximum_actions {
        let remaining = grant
            .expires_at_millis
            .saturating_sub(current_time_millis()?);
        let deadline = Instant::now() + Duration::from_millis(remaining);
        match show_ack::acknowledge_show(
            listener,
            state_dir,
            &grant,
            sequence,
            deadline,
            refreshes < MAX_FACE_REFRESHES,
        )? {
            show_ack::ShowGate::Accepted => {}
            show_ack::ShowGate::Refreshed(next) => {
                grant = *next;
                refreshes += 1;
                sequence = 1;
                continue;
            }
            _ => break,
        }
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
        eprintln!("CONDUIT_OWNER_RETURN_DIAGNOSTIC {{\"phase\":\"accepted\"}}");
        line.set_read_timeout(Some(left.min(Duration::from_secs(5))))
            .map_err(|error| format!("bound native return read: {error:?}"))?;
        let document = receive_document::<serde_json::Value>(&mut line, &mut frame, &mut assembled);
        let schema = document
            .as_ref()
            .ok()
            .and_then(|value| value.get("schema"))
            .and_then(serde_json::Value::as_str);
        if matches!(
            schema,
            Some(component_selection::SCHEMA | refresh::REFRESH_SCHEMA)
        ) {
            if refreshes >= MAX_FACE_REFRESHES {
                break;
            }
            let gate = if schema == Some(refresh::REFRESH_SCHEMA) {
                refresh::serve(
                    &mut line,
                    listener,
                    state_dir,
                    &grant,
                    sequence,
                    document.map_err(|_| "native refresh frame unavailable")?,
                )?
            } else {
                component_selection::serve(
                    &mut line,
                    listener,
                    state_dir,
                    &grant,
                    sequence,
                    document.map_err(|_| "native component frame unavailable")?,
                )?
            };
            match gate {
                show_ack::ShowGate::Refreshed(next) => {
                    grant = *next;
                    refreshes += 1;
                    sequence = 1;
                    continue;
                }
                _ => break,
            }
        }
        let action = document.and_then(|value| {
            serde_json::from_value::<Action>(value)
                .map_err(|error| format!("decode native action: {error}"))
        });
        let response = match action {
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
                action::apply(
                    listener,
                    state_dir,
                    &grant,
                    action,
                    sequence < grant.maximum_actions,
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
        eprintln!(
            "CONDUIT_OWNER_RETURN_DIAGNOSTIC {{\"phase\":\"responding\",\"code\":\"{}\"}}",
            response.code
        );
        super::response_document::send(&mut line, &action::fit_response(response)?)?;
        if outcome_unknown {
            break;
        }
        sequence += 1;
    }
    Ok(())
}

fn receive_document<T: serde::de::DeserializeOwned>(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    frame: &mut [u8],
    assembled: &mut ChunkAssembly,
) -> Result<T, String> {
    assembled.reset();
    // Each frame carries the same finite total and digest plus its exact offset.
    // The WebSocket Line provides ordering and pinned TLS; these checks keep
    // malformed or interrupted multi-frame actions distinct from submission.
    for _ in 0..MAX_RETURN_CHUNKS {
        let length = line
            .receive_binary(frame)
            .map_err(|error| format!("receive native return: {error:?}"))?;
        if assembled.expected.is_none() {
            eprintln!("CONDUIT_OWNER_RETURN_DIAGNOSTIC {{\"phase\":\"first-frame\"}}");
        }
        if assembled.push(&frame[..length])? {
            let action = serde_json::from_slice(&assembled.bytes)
                .map_err(|error| format!("decode native return: {error}"))?;
            return Ok(action);
        }
    }
    Err("native return chunk count exceeded".into())
}

struct ChunkAssembly {
    bytes: Vec<u8>,
    expected: Option<(usize, [u8; 32])>,
}

impl Default for ChunkAssembly {
    fn default() -> Self {
        Self {
            bytes: Vec::with_capacity(MAX_RETURN_ACTION_BYTES),
            expected: None,
        }
    }
}

impl ChunkAssembly {
    fn reset(&mut self) {
        self.bytes.clear();
        self.expected = None;
    }

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

#[cfg(test)]
#[path = "return_route/tests.rs"]
mod tests;
