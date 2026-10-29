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
fn abandoned_response_releases_storage_at_expiry_without_more_http_input() {
    use conduit_environment_http::face_export::Request;
    let request = Request::Snapshot { nonce: [1; 32] };
    let (_, memory) = allocation_probe::observe(|| {
        let mut state = export_state::State {
            pending: Some(request),
            completed: Some((request, 5000, Ok(vec![0; 1408]))),
        };
        state.expire(4999);
        assert!(state.completed.is_some());
        state.expire(5000);
        assert!(state.completed.is_none());
        assert_eq!(state.pending, Some(request));
        state.expire(u64::MAX);
        assert!(state.completed.is_none());
    });
    assert!(memory.peak_bytes >= 1408);
    assert_eq!(memory.retained_bytes(), 0);
}
