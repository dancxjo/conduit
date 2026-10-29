//! CPU proof of the exact native serializer and bounded response retirement.
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_face_export_state.rs"]
mod export_state;
#[path = "support/mask.rs"]
mod fixture;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_json.rs"]
mod json;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_face_stream.rs"]
mod stream;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use sha2::{Digest, Sha256};

#[test]
fn complete_face_is_canonical_and_every_export_step_allocates_nothing() {
    let face = fixture::face();
    face.validate().unwrap();
    let expected = serde_json::to_vec(&face).unwrap();
    let (descriptor, memory) = allocation_probe::observe(|| stream::snapshot(&face).unwrap());
    assert_eq!(memory.allocations + memory.reallocations, 0);
    assert_eq!(descriptor.bytes as usize, expected.len());
    assert_eq!(
        descriptor.digest,
        <[u8; 32]>::from(Sha256::digest(&expected))
    );
    let mut received = Vec::new();
    for offset in (0..descriptor.bytes).step_by(stream::CHUNK_BYTES) {
        let mut output = [0; stream::CHUNK_BYTES];
        let (length, memory) = allocation_probe::observe(|| {
            stream::chunk(&face, descriptor, offset, &mut output).unwrap()
        });
        assert_eq!(memory.allocations + memory.reallocations, 0);
        received.extend_from_slice(&output[..length]);
    }
    assert_eq!(received, expected);
    let decoded: conduit_face::Face = serde_json::from_slice(&received).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded, face);
}

#[test]
fn changed_revision_or_bytes_cannot_mix_into_an_existing_snapshot() {
    let mut face = fixture::face();
    let prior = stream::snapshot(&face).unwrap();
    let mut output = [0; stream::CHUNK_BYTES];
    face.revision += 1;
    assert_eq!(
        stream::chunk(&face, prior, 0, &mut output),
        Err(stream::Refusal::Changed)
    );
    face.revision -= 1;
    face.subjects[0].name.push('!');
    assert_eq!(
        stream::chunk(&face, prior, 0, &mut output),
        Err(stream::Refusal::Changed)
    );
}

#[test]
fn finite_document_and_exact_offsets_refuse_without_growth() {
    let mut face = fixture::face();
    let descriptor = stream::snapshot(&face).unwrap();
    for offset in [1, descriptor.bytes, u32::MAX] {
        assert_eq!(
            stream::chunk(&face, descriptor, offset, &mut [0; stream::CHUNK_BYTES]),
            Err(stream::Refusal::Offset)
        );
    }
    // The export boundary independently refuses an oversized serialization;
    // this deliberately malformed fixture is never admitted or published.
    face.subjects[0].name = "x".repeat(stream::MAXIMUM_BYTES);
    let (result, memory) = allocation_probe::observe(|| stream::snapshot(&face));
    assert_eq!(result, Err(stream::Refusal::Capacity));
    assert_eq!(memory.allocations + memory.reallocations, 0);
}
#[test]
fn unsupported_constraints_refuse_instead_of_disappearing_from_the_same_face() {
    let mut face = fixture::face();
    let value = face
        .properties
        .iter_mut()
        .find_map(|p| match &mut p.value {
            conduit_face::FacePropertyValue::TypedValue { contract, .. } => Some(contract),
            _ => None,
        })
        .unwrap();
    value
        .constraints
        .push(conduit_core::ValueConstraint::ByteLength {
            minimum: 0,
            maximum: 1,
        });
    assert_eq!(stream::snapshot(&face), Err(stream::Refusal::Unsupported));
}
#[test]
fn leaf_metadata_and_escaped_text_keep_exact_canonical_bytes() {
    let mut face = fixture::face();
    // Codec coverage only: these additions deliberately do not reseal/admit a
    // new producer revision. Compare every byte, including the retained ID.
    face.relationships.push(conduit_face::FaceRelationship {
        source: "environment".into(),
        target: "environment".into(),
        kind: conduit_face::FaceRelationshipKind::Observes,
    });
    face.interaction_context
        .basis
        .push(conduit_face::FaceContextBasis {
            source: "environment".into(),
            relationship: conduit_face::FaceRelationshipKind::Observes,
            target: "environment".into(),
        });
    face.text.push(conduit_face::FaceText {
        subject: "environment".into(),
        text: "\"snow\"\n雪\t".into(),
    });
    let expected = serde_json::to_vec(&face).unwrap();
    let descriptor = stream::snapshot(&face).unwrap();
    let mut actual = Vec::new();
    for offset in (0..descriptor.bytes).step_by(stream::CHUNK_BYTES) {
        let mut bytes = [0; stream::CHUNK_BYTES];
        let n = stream::chunk(&face, descriptor, offset, &mut bytes).unwrap();
        actual.extend_from_slice(&bytes[..n]);
    }
    assert_eq!(actual, expected);
}

#[test]
fn abandoned_pending_expires_without_publication_or_http_input() {
    use conduit_environment_http::face_export::{Reply, Request};
    let request = Request::Snapshot { nonce: [1; 32] };
    let mut state = export_state::State::new();
    assert!(matches!(state.request(request, 100), Reply::Pending));
    state.expire(5100);
    assert!(state.begin(5100).is_none());
}

#[test]
fn completion_retains_only_original_admission_lifetime() {
    use conduit_environment_http::face_export::{Reply, Request};
    let first = Request::Snapshot { nonce: [1; 32] };
    let second = Request::Snapshot { nonce: [2; 32] };
    let (_, memory) = allocation_probe::observe(|| {
        let mut state = export_state::State::new();
        assert!(matches!(state.request(first, 100), Reply::Pending));
        let work = state.begin(4000).unwrap();
        state.complete(work, 4001, Ok(vec![42; 1408]));
        assert!(matches!(state.request(second, 5099), Reply::Unavailable));
        state.expire(5100);
        assert!(matches!(state.request(second, 5100), Reply::Pending));
    });
    assert!(memory.peak_bytes >= 1408);
    assert_eq!(memory.retained_bytes(), 0);
}

#[test]
fn in_flight_owner_blocks_alien_and_expired_work_cannot_publish() {
    use conduit_environment_http::face_export::{Reply, Request};
    let first = Request::Snapshot { nonce: [1; 32] };
    let second = Request::Snapshot { nonce: [2; 32] };
    let mut state = export_state::State::new();
    assert!(matches!(state.request(first, 0), Reply::Pending));
    let work = state.begin(10).unwrap();
    assert!(matches!(state.request(first, 20), Reply::Pending));
    assert!(matches!(state.request(second, 20), Reply::Unavailable));
    state.expire(5000);
    assert!(!work.is_current(5000));
    assert!(matches!(state.request(second, 5000), Reply::Unavailable));
    assert!(state.begin(5000).is_none());
    state.complete(work, 5000, Ok(vec![99]));
    assert!(matches!(state.request(second, 5000), Reply::Pending));
    let next = state.begin(5001).unwrap();
    assert!(matches!(state.request(first, 5002), Reply::Unavailable));
    state.complete(next, 5003, Ok(vec![7]));
    assert!(matches!(state.request(second, 5004), Reply::Offer(bytes) if bytes == vec![7]));
}

#[test]
fn exact_retry_collects_once_without_extending_deadline() {
    use conduit_environment_http::face_export::{Reply, Request};
    let request = Request::Snapshot { nonce: [3; 32] };
    let mut state = export_state::State::new();
    assert!(matches!(state.request(request, 0), Reply::Pending));
    assert!(matches!(state.request(request, 4900), Reply::Pending));
    let work = state.begin(4901).unwrap();
    assert!(work.is_current(4999));
    assert!(!work.is_current(5000));
    state.complete(work, 4998, Ok(vec![1, 2]));
    assert!(matches!(state.request(request, 4999), Reply::Offer(bytes) if bytes == vec![1, 2]));
    assert!(state.begin(4999).is_none());
}

#[test]
fn expired_actual_work_does_not_invoke_signing_callback() {
    use conduit_environment_http::face_export::{Reply, Request};
    let request = Request::Chunk {
        digest: [4; 32],
        revision_le: 9u64.to_le_bytes(),
        bytes: 512,
        offset: 0,
    };
    let mut state = export_state::State::new();
    assert!(matches!(state.request(request, 0), Reply::Pending));
    let work = state.begin(1).unwrap();
    assert_eq!(work.request(), request);
    let calls = core::cell::Cell::new(0);
    assert_eq!(
        work.while_current(4999, || {
            calls.set(calls.get() + 1);
            Ok(7)
        }),
        Ok(7)
    );
    assert_eq!(
        work.while_current(5000, || {
            calls.set(calls.get() + 1);
            Ok(8)
        }),
        Err("face-export-expired")
    );
    assert_eq!(calls.get(), 1);
    state.complete(work, 5001, Ok(vec![7]));
    assert!(state.begin(5001).is_none());
}
