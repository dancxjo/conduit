//! An acknowledged graphical Show followed by one bounded semantic action.
//! Each uses a fresh pinned TLS Line under the return grant; the spent
//! invitation is never replayed or treated as a reconnect key.

use alloc::{string::String, vec, vec::Vec};
use conduit_body::PortableAdmissionReceipt;
use conduit_presentation::{
    FaceInteraction, MaskShow, OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse,
    OWNER_FACE_REQUEST_SCHEMA,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    arch::{CandidateDeadline, VirtioNetReady},
    bounded_websocket::{BinaryWebSocketIo, MAXIMUM_BINARY_MESSAGE_BYTES},
    identity::BootIdentities,
    native_guest_face::NativeGuestFace,
    native_owner_admission::NativeOwnerReturnGrant,
    native_owner_document,
    virtio_tcp::VirtioTcpEndpoint,
    virtio_tls::{self, VirtioWebSocketRunError},
};

const ACTION_SCHEMA: &str = "conduit.body/native-owner-return-action@1";
const RESPONSE_SCHEMA: &str = "conduit.body/native-owner-return-response@1";
const SHOW_ACK_SCHEMA: &str = "conduit.body/native-owner-show-ack@1";
const SHOW_ACK_RESPONSE_SCHEMA: &str = "conduit.body/native-owner-show-ack-response@1";
#[path = "native_owner_return/refresh.rs"]
mod refresh;
// Two bounded actor calls (action and refreshed Face) may follow the accepted
// submission. Their outcome window is separate from the grant's input cutoff.
const MAX_ACTION_MILLIS: u32 = 10_000;
const MAXIMUM_POLLS: u32 = 1_000_000;
const MAX_RETURN_ACTION_BYTES: usize = 64 * 1024;
const CHUNK_HEADER_BYTES: usize = 40;

pub struct NativeOwnerReturnRoute {
    grant: NativeOwnerReturnGrant,
    receipt: PortableAdmissionReceipt,
    device: Option<VirtioNetReady>,
    endpoint: VirtioTcpEndpoint,
    server_identity: String,
    certificate_der: Vec<u8>,
    lifetime: CandidateDeadline,
    next_sequence: u8,
    refreshes: u8,
    active: bool,
    acknowledged_show: Option<MaskShow>,
    action_storage: Vec<u8>,
    frame_storage: Vec<u8>,
}

#[derive(Serialize)]
struct Action<'a> {
    schema: &'static str,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    show: &'a MaskShow,
    interaction: &'a FaceInteraction,
}

#[derive(Serialize)]
struct ShowAcknowledgement<'a> {
    schema: &'static str,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    show: &'a MaskShow,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShowAcknowledgementResponse {
    schema: String,
    accepted: bool,
    code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    schema: String,
    accepted: bool,
    code: String,
    face: Option<OwnerFaceSnapshotResponse>,
}

pub struct NativeReturnOutcome {
    pub accepted: bool,
    pub code: String,
    pub face: Option<NativeGuestFace>,
}

impl NativeOwnerReturnRoute {
    pub fn admit(
        grant: NativeOwnerReturnGrant,
        receipt: PortableAdmissionReceipt,
        device: VirtioNetReady,
        endpoint: VirtioTcpEndpoint,
        server_identity: String,
        certificate_der: Vec<u8>,
    ) -> Result<Self, &'static str> {
        let lifetime = CandidateDeadline::admit(grant.remaining_millis)
            .ok_or("native-return-clock-unavailable")?;
        Ok(Self {
            grant,
            receipt,
            device: Some(device),
            endpoint,
            server_identity,
            certificate_der,
            lifetime,
            next_sequence: 1,
            refreshes: 0,
            active: true,
            acknowledged_show: None,
            action_storage: vec![0; MAX_RETURN_ACTION_BYTES],
            frame_storage: vec![0; MAXIMUM_BINARY_MESSAGE_BYTES],
        })
    }

    pub fn available(&self) -> bool {
        self.active
            && self.device.is_some()
            && self.next_sequence <= self.grant.maximum_actions
            && self
                .lifetime
                .elapsed_millis()
                .is_some_and(|elapsed| elapsed < i64::from(self.grant.remaining_millis))
    }

    pub fn acknowledge_show(
        &mut self,
        identities: BootIdentities,
        show: &MaskShow,
    ) -> Result<(), &'static str> {
        if !self.available() || self.acknowledged_show.is_some() {
            return Err("native-owner-show-ack-unavailable");
        }
        self.active = false;
        let sequence = self.next_sequence;
        let deadline = CandidateDeadline::admit(MAX_ACTION_MILLIS)
            .ok_or("native-owner-return-clock-unavailable")?;
        let endpoint = next_return_endpoint(&mut self.endpoint)?;
        let seeds = crate::native_boot_join::fresh_seeds()?;
        let device = self
            .device
            .take()
            .ok_or("native-owner-return-device-unavailable")?;
        if device.identity().boot_id != identities.boot {
            return Err("native-owner-return-stale-boot");
        }
        let request = owner_request(&self.receipt);
        let length = serde_json_core::to_slice(
            &ShowAcknowledgement {
                schema: SHOW_ACK_SCHEMA,
                token: self.grant.token,
                sequence,
                request,
                show,
            },
            &mut self.action_storage,
        )
        .map_err(|_| "native-owner-show-ack-pressure")?;
        if length == 0 || length > MAX_RETURN_ACTION_BYTES {
            return Err("native-owner-show-ack-pressure");
        }
        let encoded = &self.action_storage[..length];
        let (response, _, device) = virtio_tls::with_websocket_deadline_retain_device(
            device,
            seeds.tcp,
            seeds.tls,
            seeds.websocket,
            endpoint,
            &self.server_identity,
            &self.certificate_der,
            MAXIMUM_POLLS,
            Some(deadline),
            |line| exchange_show_ack(line, encoded, &mut self.frame_storage),
        )
        .map_err(|_| "control-outcome-unknown")?;
        self.device = Some(device);
        if response.schema != SHOW_ACK_RESPONSE_SCHEMA
            || response.code != "accepted"
            || !response.accepted
        {
            return Err("native-owner-show-ack-refused");
        }
        self.acknowledged_show = Some(show.clone());
        self.active = true;
        Ok(())
    }

    pub fn submit(
        &mut self,
        identities: BootIdentities,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<NativeReturnOutcome, &'static str> {
        if !self.available() || self.acknowledged_show.as_ref() != Some(show) {
            return Err("native-owner-return-unavailable");
        }
        self.acknowledged_show = None;
        let sequence = self.next_sequence;
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or("native-owner-return-action-bound")?;
        // An uncertain transport outcome consumes the local grant position.
        // The owner may have acted; the native guest must not retry blindly.
        self.active = false;
        let deadline = CandidateDeadline::admit(MAX_ACTION_MILLIS)
            .ok_or("native-owner-return-clock-unavailable")?;
        let endpoint = next_return_endpoint(&mut self.endpoint)?;
        let seeds = crate::native_boot_join::fresh_seeds()?;
        let device = self
            .device
            .take()
            .ok_or("native-owner-return-device-unavailable")?;
        if device.identity().boot_id != identities.boot {
            return Err("native-owner-return-stale-boot");
        }
        let request = owner_request(&self.receipt);
        let length = serde_json_core::to_slice(
            &Action {
                schema: ACTION_SCHEMA,
                token: self.grant.token,
                sequence,
                request,
                show,
                interaction,
            },
            &mut self.action_storage,
        )
        .map_err(|_| "native-owner-return-action-pressure")?;
        if length == 0 {
            return Err("native-owner-return-encoding-invalid");
        }
        let encoded = &self.action_storage[..length];
        let (response, _, device) = virtio_tls::with_websocket_deadline_retain_device(
            device,
            seeds.tcp,
            seeds.tls,
            seeds.websocket,
            endpoint,
            &self.server_identity,
            &self.certificate_der,
            MAXIMUM_POLLS,
            Some(deadline),
            |line| exchange(line, encoded, &mut self.frame_storage),
        )
        .map_err(|error| {
            let (phase, code) = match error {
                VirtioWebSocketRunError::Transport(error) => ("transport", error.as_str()),
                VirtioWebSocketRunError::Operation(code) => ("exchange", code),
            };
            crate::arch::early_write(b"CONDUIT_NATIVE_OWNER_RETURN_DIAGNOSTIC {\"phase\":\"");
            crate::arch::early_write(phase.as_bytes());
            crate::arch::early_write(b"\",\"code\":\"");
            crate::arch::early_write(code.as_bytes());
            crate::arch::early_write(b"\"}\n");
            "control-outcome-unknown"
        })?;
        self.device = Some(device);
        if response.schema != RESPONSE_SCHEMA
            || response.code.is_empty()
            || response.code.len() > 256
        {
            return Err("control-outcome-unknown");
        }
        let face = response
            .face
            .map(|face| {
                NativeGuestFace::from_owner_response(&self.receipt, face)
                    .map_err(|_| "control-outcome-unknown")
            })
            .transpose()?;
        self.active = self.next_sequence <= self.grant.maximum_actions
            && response.code != "control-outcome-unknown"
            && self.lifetime.elapsed_millis().is_some()
            && face
                .as_ref()
                .is_some_and(NativeGuestFace::interactions_admitted);
        Ok(NativeReturnOutcome {
            accepted: response.accepted,
            code: response.code,
            face,
        })
    }
}

fn owner_request(receipt: &PortableAdmissionReceipt) -> OwnerFaceSnapshotRequest {
    let credential = &receipt.credential;
    OwnerFaceSnapshotRequest {
        schema: OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: credential.credential_id.as_str().into(),
        body_id: credential.body_id.clone(),
        part_id: credential.part_id.clone(),
        host_id: credential.host_id.clone(),
        boot_id: credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    }
}

fn exchange_show_ack(
    line: &mut dyn BinaryWebSocketIo,
    encoded: &[u8],
    frame: &mut [u8],
) -> Result<ShowAcknowledgementResponse, &'static str> {
    send_chunks(line, encoded, frame)?;
    let mut response = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
    let length = line
        .receive_binary(&mut response)
        .map_err(|_| "native-owner-show-ack-receive-refused")?;
    serde_json::from_slice(&response[..length]).map_err(|_| "native-owner-show-ack-decode-refused")
}

fn exchange(
    line: &mut dyn BinaryWebSocketIo,
    encoded: &[u8],
    frame: &mut [u8],
) -> Result<Response, &'static str> {
    send_chunks(line, encoded, frame)?;
    native_owner_document::receive(line, MAX_RETURN_ACTION_BYTES).map_err(|error| {
        let code = match error {
            native_owner_document::DocumentRefusal::Receive(error) => error.as_str(),
            native_owner_document::DocumentRefusal::Bound => "native-owner-return-document-bound",
            native_owner_document::DocumentRefusal::Decode => "native-owner-return-document-decode",
        };
        crate::arch::early_write(b"CONDUIT_NATIVE_OWNER_RETURN_DOCUMENT {\"code\":\"");
        crate::arch::early_write(code.as_bytes());
        crate::arch::early_write(b"\"}\n");
        "control-outcome-unknown"
    })
}

fn send_chunks(
    line: &mut dyn BinaryWebSocketIo,
    encoded: &[u8],
    frame: &mut [u8],
) -> Result<(), &'static str> {
    if encoded.is_empty()
        || encoded.len() > MAX_RETURN_ACTION_BYTES
        || frame.len() != MAXIMUM_BINARY_MESSAGE_BYTES
    {
        return Err("native-owner-return-action-pressure");
    }
    let total = u32::try_from(encoded.len()).map_err(|_| "native-owner-return-action-pressure")?;
    let digest = Sha256::digest(encoded);
    let payload_capacity = frame.len() - CHUNK_HEADER_BYTES;
    for (index, chunk) in encoded.chunks(payload_capacity).enumerate() {
        let offset = index * payload_capacity;
        frame[..4].copy_from_slice(&total.to_le_bytes());
        frame[4..8].copy_from_slice(&(offset as u32).to_le_bytes());
        frame[8..CHUNK_HEADER_BYTES].copy_from_slice(&digest);
        frame[CHUNK_HEADER_BYTES..CHUNK_HEADER_BYTES + chunk.len()].copy_from_slice(chunk);
        line.send_binary(&frame[..CHUNK_HEADER_BYTES + chunk.len()])
            .map_err(|_| "native-owner-return-send-refused")?;
    }
    Ok(())
}

// TCP source ports identify connections, independently from replaceable grants.
// Consume a fresh port even on uncertain outcomes; never retry an old tuple.
fn next_return_endpoint(
    endpoint: &mut VirtioTcpEndpoint,
) -> Result<VirtioTcpEndpoint, &'static str> {
    endpoint.local_port = endpoint
        .local_port
        .checked_add(1)
        .ok_or("native-owner-return-port-bound")?;
    Ok(*endpoint)
}

#[cfg(test)]
#[path = "native_owner_return/tests.rs"]
mod tests;
