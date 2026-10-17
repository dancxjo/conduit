//! Native return connection and exact typed wire-envelope contracts.

use super::*;
use crate::bounded_websocket::WebSocketError;
use conduit_face::{FaceInteractionArgument, UTF8_TEXT_VALUE_KIND};

#[test]
fn return_connections_never_reuse_ports_after_grant_sequence_reset() {
    let mut endpoint = VirtioTcpEndpoint {
        guest_address: [10, 0, 2, 15],
        prefix_length: 24,
        gateway: [10, 0, 2, 2],
        remote_address: [10, 0, 2, 42],
        remote_port: 4433,
        local_port: 50000,
    };
    // F5 refresh/ack followed by three component selections/acks, then action.
    for expected in 50001..=50009 {
        let next = next_return_endpoint(&mut endpoint).unwrap();
        assert_eq!(next.local_port, expected);
        assert_eq!(next.remote_port, 4433);
    }
    endpoint.local_port = u16::MAX;
    assert_eq!(
        next_return_endpoint(&mut endpoint),
        Err("native-owner-return-port-bound")
    );
    assert_eq!(endpoint.local_port, u16::MAX);
}

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
    assert_eq!(line.0.len(), 2);
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
    assert!(length > 0 && length <= MAX_RETURN_ACTION_BYTES);
    let mut line = RecordedLine::default();
    let mut frame = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
    send_chunks(&mut line, &storage[..length], &mut frame).unwrap();
    let reassembled: Vec<u8> = line
        .0
        .iter()
        .flat_map(|chunk| chunk[CHUNK_HEADER_BYTES..].iter().copied())
        .collect();
    assert_eq!(reassembled, storage[..length]);
    assert!(
        line.0
            .iter()
            .all(|chunk| chunk.len() <= MAXIMUM_BINARY_MESSAGE_BYTES)
    );
    assert_eq!(decoded.schema, ACTION_SCHEMA);
    assert_eq!(decoded.token, [7; 32]);
    assert_eq!(decoded.sequence, 1);
    assert_eq!(decoded.request, request);
    assert_eq!(decoded.show, show);
    assert_eq!(decoded.interaction, interaction);

    let acknowledgement_length = serde_json_core::to_slice(
        &ShowAcknowledgement {
            schema: SHOW_ACK_SCHEMA,
            token: [7; 32],
            sequence: 1,
            request: decoded.request,
            show: &show,
        },
        &mut storage,
    )
    .unwrap();
    assert!(acknowledgement_length <= MAX_RETURN_ACTION_BYTES);
    let mut acknowledged_line = RecordedLine::default();
    send_chunks(
        &mut acknowledged_line,
        &storage[..acknowledgement_length],
        &mut frame,
    )
    .unwrap();
    assert!(acknowledged_line.0.len() <= 16);
}
