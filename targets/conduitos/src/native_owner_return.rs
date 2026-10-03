//! One bounded semantic action from the graphical native Mask to its owner.
//! A fresh TLS Line uses a grant received over the admitted invitation Line;
//! the invitation itself is never replayed or treated as a reconnect key.

use alloc::{string::String, vec, vec::Vec};
use conduit_body::PortableAdmissionReceipt;
use conduit_presentation::{
    FaceInteraction, MaskShow, OWNER_FACE_REQUEST_SCHEMA, OwnerFaceSnapshotRequest,
    OwnerFaceSnapshotResponse,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    arch::{CandidateDeadline, VirtioNetReady},
    bounded_websocket::{BinaryWebSocketIo, MAXIMUM_BINARY_MESSAGE_BYTES},
    identity::BootIdentities,
    native_guest_face::NativeGuestFace,
    native_owner_admission::NativeOwnerReturnGrant,
    virtio_tcp::VirtioTcpEndpoint,
    virtio_tls::{self, VirtioWebSocketRunError},
};

const ACTION_SCHEMA: &str = "conduit.body/native-owner-return-action@1";
const RESPONSE_SCHEMA: &str = "conduit.body/native-owner-return-response@1";
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
    active: bool,
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
            active: true,
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

    pub fn submit(
        &mut self,
        identities: BootIdentities,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<NativeReturnOutcome, &'static str> {
        if !self.available() {
            return Err("native-owner-return-unavailable");
        }
        let sequence = self.next_sequence;
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or("native-owner-return-action-bound")?;
        // An uncertain transport outcome consumes the local grant position.
        // The owner may have acted; the native guest must not retry blindly.
        self.active = false;
        let deadline = CandidateDeadline::admit(MAX_ACTION_MILLIS)
            .ok_or("native-owner-return-clock-unavailable")?;
        let mut endpoint = self.endpoint;
        endpoint.local_port = endpoint
            .local_port
            .checked_add(u16::from(sequence))
            .ok_or("native-owner-return-port-bound")?;
        let seeds = crate::native_boot_join::fresh_seeds()?;
        let device = self
            .device
            .take()
            .ok_or("native-owner-return-device-unavailable")?;
        if device.identity().boot_id != identities.boot {
            return Err("native-owner-return-stale-boot");
        }
        let credential = &self.receipt.credential;
        let request = OwnerFaceSnapshotRequest {
            schema: OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: credential.credential_id.as_str().into(),
            body_id: credential.body_id.clone(),
            part_id: credential.part_id.clone(),
            host_id: credential.host_id.clone(),
            boot_id: credential.boot_id.clone(),
            last_seen_revision: None,
            last_seen_identity: None,
        };
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

fn exchange(
    line: &mut dyn BinaryWebSocketIo,
    encoded: &[u8],
    frame: &mut [u8],
) -> Result<Response, &'static str> {
    send_chunks(line, encoded, frame)?;
    let mut bytes = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
    let length = line
        .receive_binary(&mut bytes)
        .map_err(|_| "native-owner-return-receive-refused")?;
    if length == 0 || length > bytes.len() {
        return Err("control-outcome-unknown");
    }
    serde_json::from_slice(&bytes[..length]).map_err(|_| "control-outcome-unknown")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_websocket::WebSocketError;
    use conduit_presentation::{FaceInteractionArgument, UTF8_TEXT_VALUE_KIND};

    #[derive(Default)]
    struct RecordedLine(Vec<Vec<u8>>);

    impl BinaryWebSocketIo for RecordedLine {
        fn send_binary(&mut self, payload: &[u8]) -> Result<(), WebSocketError> {
            self.0.push(payload.into());
            Ok(())
        }

        fn receive_binary(&mut self, _: &mut [u8]) -> Result<usize, WebSocketError> {
            Err(WebSocketError::UnexpectedFrame)
        }
    }

    #[test]
    fn action_chunks_are_finite_ordered_and_digest_bound() {
        let data = vec![0x5a; 20_000];
        let mut frame = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
        let mut line = RecordedLine::default();
        send_chunks(&mut line, &data, &mut frame).unwrap();
        assert_eq!(line.0.len(), 3);
        let digest = Sha256::digest(&data);
        for (index, chunk) in line.0.iter().enumerate() {
            assert!(chunk.len() <= MAXIMUM_BINARY_MESSAGE_BYTES);
            assert_eq!(u32::from_le_bytes(chunk[..4].try_into().unwrap()), 20_000);
            assert_eq!(
                u32::from_le_bytes(chunk[4..8].try_into().unwrap()) as usize,
                index * (MAXIMUM_BINARY_MESSAGE_BYTES - CHUNK_HEADER_BYTES)
            );
            assert_eq!(&chunk[8..CHUNK_HEADER_BYTES], digest.as_slice());
        }
        assert_eq!(
            send_chunks(&mut line, &vec![0; MAX_RETURN_ACTION_BYTES + 1], &mut frame),
            Err("native-owner-return-action-pressure")
        );
    }

    #[test]
    fn exact_checked_show_and_interaction_survive_bounded_action_envelope() {
        let face = crate::native_face_scene::tests::face(12);
        let show = crate::native_face_scene::tests::fixture::show(&face);
        let interaction = FaceInteraction::new(
            &face,
            &show,
            "edit",
            "a-field",
            vec![FaceInteractionArgument {
                name: "value".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: b"500".into(),
            }],
            1,
        )
        .unwrap();
        let request = OwnerFaceSnapshotRequest {
            schema: OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: "credential/native-fixture".into(),
            body_id: serde_json::from_str("\"body/native-fixture\"").unwrap(),
            part_id: serde_json::from_str("\"part/native-fixture\"").unwrap(),
            host_id: "host/native-fixture".into(),
            boot_id: "boot/native-fixture".into(),
            last_seen_revision: None,
            last_seen_identity: None,
        };
        let mut storage = vec![0; MAX_RETURN_ACTION_BYTES];
        let length = serde_json_core::to_slice(
            &Action {
                schema: ACTION_SCHEMA,
                token: [7; 32],
                sequence: 1,
                request: request.clone(),
                show: &show,
                interaction: &interaction,
            },
            &mut storage,
        )
        .unwrap();
        #[derive(Deserialize)]
        struct Decoded {
            schema: String,
            token: [u8; 32],
            sequence: u8,
            request: OwnerFaceSnapshotRequest,
            show: MaskShow,
            interaction: FaceInteraction,
        }
        let decoded: Decoded = serde_json::from_slice(&storage[..length]).unwrap();
        assert!(length > MAXIMUM_BINARY_MESSAGE_BYTES);
        let mut line = RecordedLine::default();
        let mut frame = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
        send_chunks(&mut line, &storage[..length], &mut frame).unwrap();
        assert!(line.0.len() >= 2);
        assert_eq!(decoded.schema, ACTION_SCHEMA);
        assert_eq!(decoded.token, [7; 32]);
        assert_eq!(decoded.sequence, 1);
        assert_eq!(decoded.request, request);
        assert_eq!(decoded.show, show);
        assert_eq!(decoded.interaction, interaction);
    }
}
