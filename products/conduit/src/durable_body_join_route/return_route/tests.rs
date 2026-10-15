use super::*;
use conduit_face::OWNER_FACE_REQUEST_SCHEMA;

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

#[test]
fn face_replacement_preserves_deadline_and_consumed_action_budget() {
    let original = Grant::issue(&receipt(), MaskSelection::Native).unwrap();
    let terminal = Grant::issue(&receipt(), MaskSelection::Terminal).unwrap();
    assert_eq!(
        terminal.replacement(2).unwrap().mask,
        MaskSelection::Terminal
    );
    let mut current = original.replacement(3).unwrap();
    assert_eq!(current.maximum_actions, 2);
    for _ in 0..MAX_FACE_REFRESHES {
        let next = current.replacement(1).unwrap();
        assert_ne!(next.token, current.token);
        assert_eq!(next.expires_at_millis, original.expires_at_millis);
        assert!(next.remaining_millis <= current.remaining_millis);
        assert_eq!(next.maximum_actions, 2);
        assert!(next.matches_request(next.token, &request()));
        assert!(!next.matches_request(current.token, &request()));
        current = next;
    }
    assert!(current.replacement(0).is_err());
    assert!(current.replacement(4).is_err());
    current.expires_at_millis = 0;
    assert!(current.replacement(1).is_err());
}

fn receipt() -> PortableAdmissionReceipt {
    let request = request();
    PortableAdmissionReceipt {
        schema: conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential: conduit_body::MembershipCredential {
            credential_id: serde_json::from_str("\"credential/guest\"").unwrap(),
            body_id: request.body_id,
            part_id: request.part_id,
            host_id: request.host_id.clone(),
            boot_id: request.boot_id.clone(),
            issued_at_millis: 1,
        },
        host_advertisement: conduit_core::HostAdvertisement {
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: request.host_id,
            boot_id: request.boot_id,
            offer_generation: conduit_core::OfferGeneration(1),
            profile: conduit_core::HostProfileId::from("test"),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        },
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    }
}

fn chunks_at(payload: &[u8], frame_bytes: usize) -> Vec<Vec<u8>> {
    let digest = Sha256::digest(payload);
    payload
        .chunks(frame_bytes - CHUNK_HEADER_BYTES)
        .enumerate()
        .map(|(index, chunk)| {
            let mut frame = Vec::with_capacity(CHUNK_HEADER_BYTES + chunk.len());
            frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            frame.extend_from_slice(
                &((index * (frame_bytes - CHUNK_HEADER_BYTES)) as u32).to_le_bytes(),
            );
            frame.extend_from_slice(&digest);
            frame.extend_from_slice(chunk);
            frame
        })
        .collect()
}

#[test]
fn native_eight_kib_frames_cover_the_full_bounded_return() {
    let payload = vec![0x5a; MAX_RETURN_ACTION_BYTES];
    let frames = chunks_at(&payload, 8192 - 14);
    assert!(frames.len() <= MAX_RETURN_CHUNKS);
    assert!(frames.len() > 2);
    let mut assembly = ChunkAssembly::default();
    for frame in &frames[..frames.len() - 1] {
        assert!(!assembly.push(frame).unwrap());
    }
    assert!(assembly.push(frames.last().unwrap()).unwrap());
    assert_eq!(assembly.bytes, payload);
}

#[test]
fn chunk_assembly_accepts_large_exact_payload_and_refuses_offset_digest_and_pressure() {
    // Return actions retain their own bound when the owner Face profile grows.
    let payload = vec![0x5a; MAX_RETURN_ACTION_BYTES];
    let frames = chunks_at(&payload, 32 * 1024);
    assert_eq!(frames.len(), 3);
    let mut assembly = ChunkAssembly::default();
    assert!(!assembly.push(&frames[0]).unwrap());
    assert!(!assembly.push(&frames[1]).unwrap());
    assert!(assembly.push(&frames[2]).unwrap());
    assert_eq!(assembly.bytes, payload);
    assert!(assembly.bytes.capacity() >= MAX_RETURN_ACTION_BYTES);
    assembly.reset();
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
    assert_eq!(
        decode_face_request(&wrapped).unwrap().2,
        MaskSelection::Native
    );
    let terminal = serde_json::to_vec(&serde_json::json!({
        "schema": FACE_REQUEST_SCHEMA, "request": request(), "mask": "terminal",
    }))
    .unwrap();
    assert_eq!(
        decode_face_request(&terminal).unwrap().2,
        MaskSelection::Terminal
    );
    let unknown = serde_json::to_vec(&serde_json::json!({
        "schema": FACE_REQUEST_SCHEMA, "request": request(), "mask": "unknown",
    }))
    .unwrap();
    assert!(decode_face_request(&unknown).is_err());
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
        receipt: receipt(),
        mask: MaskSelection::Native,
        spoken_selection: None,
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
fn oversized_refreshed_face_keeps_action_truth_and_refuses_document_pressure() {
    let response = Response {
        schema: RESPONSE_SCHEMA,
        accepted: true,
        code: "accepted",
        face: Some(OwnerFaceSnapshotResponse::Refused {
            schema: "conduit.face/owner-face-response@1".into(),
            code: "x".repeat(super::super::response_document::MAX_OWNER_DOCUMENT_BYTES),
        }),
    };
    let fitted = action::fit_response(response).unwrap();
    assert!(fitted.accepted);
    assert_eq!(fitted.code, "accepted-face-pressure");
    assert!(fitted.face.is_none());
    assert!(serde_json::to_vec(&fitted).unwrap().len() <= MAX_OWNER_FACE_RESPONSE_BYTES);
}
