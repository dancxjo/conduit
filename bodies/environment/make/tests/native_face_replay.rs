//! CPU replay comparison; fixture provenance confers no native authority.
#[path = "../examples/native_owner/face_replay.rs"]
mod replay;
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedSourceFrameDecoder},
    publication::{PublicationChannel, PublicationSnapshot},
    retained_publication_face::RetainedPublicationFace,
};
use conduit_face::{Face, FaceBasis, FacePropertyValue};
use replay::{Committed, Replay};
fn basis() -> FaceBasis {
    FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some("source/CPU-fixture".into()),
        checked_plot_id: Some("checked/CPU-fixture".into()),
        expanded_plot_id: Some("expanded/CPU-fixture".into()),
        plan_id: Some("plan/CPU-fixture".into()),
        active_play_id: Some("play/CPU-fixture".into()),
        sign_ids: vec![],
    }
}
fn reading() -> Vec<u8> {
    PreparedSourceFrameDecoder::prepare(&ObservationContract::prepare().unwrap())
        .unwrap()
        .decode(5, [53, 0, 20, 2, 75])
        .unwrap()
        .to_vec()
}
fn report(face: &Face, sequence: u64) -> serde_json::Value {
    let property = |name| {
        face.properties
            .iter()
            .find(|p| p.name == name)
            .map(|p| &p.value)
    };
    let digest: Vec<u8> = face
        .identity
        .as_str()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    serde_json::json!({
        "source_sequence":sequence, "revision":face.revision, "content_digest":digest,
        "publisher_retained":true, "refusal":null,
        "fresh":matches!(property("fresh"), Some(FacePropertyValue::Flag(true))),
        "available":matches!(property("sensor-status"), Some(FacePropertyValue::Text(t)) if t == "available"),
    })
}
fn committed(bytes: &[u8], sequence: u64, commit: u64, observed: u64) -> Committed<'_> {
    Committed {
        channel: 0,
        sequence,
        canonical: bytes,
        committed_ms: commit,
        observed_ms: observed,
    }
}
fn publish(owner: &mut RetainedPublicationFace, bytes: &[u8], sequence: u64, time: u64) {
    owner
        .publish(
            PublicationSnapshot {
                channel: PublicationChannel::Reading,
                sequence,
                canonical: bytes,
            },
            time,
        )
        .unwrap();
}
#[test]
fn verifies_new_and_repeated_publications_freshness_and_recovery() {
    let bytes = reading();
    let mut expected = RetainedPublicationFace::prepare(basis(), 3, 100).unwrap();
    let mut replay = Replay::prepare(basis(), 3, 100).unwrap();
    publish(&mut expected, &bytes, 1, 1000);
    for observed in [1001, 1002, 1200] {
        let face = expected.at(observed).unwrap();
        assert_eq!(
            replay
                .verify(committed(&bytes, 1, 1000, observed), &report(face, 1))
                .unwrap(),
            face
        );
    }
    publish(&mut expected, &bytes, 2, 1300);
    assert_eq!(
        replay
            .verify(
                committed(&bytes, 2, 1300, 1301),
                &report(expected.current(), 2)
            )
            .unwrap(),
        expected.current()
    );
}
#[test]
fn refuses_changed_summary_scope_and_same_sequence_publication() {
    let bytes = reading();
    let mut expected = RetainedPublicationFace::prepare(basis(), 3, 100).unwrap();
    publish(&mut expected, &bytes, 1, 1000);
    let valid = report(expected.current(), 1);
    for (field, value) in [
        ("source_sequence", serde_json::json!(2)),
        ("revision", serde_json::json!(999)),
        ("content_digest", serde_json::json!([0; 32].to_vec())),
        ("publisher_retained", serde_json::json!(false)),
        ("fresh", serde_json::json!(false)),
        ("available", serde_json::json!(false)),
        ("refusal", serde_json::json!("structured")),
    ] {
        let mut changed = valid.clone();
        changed[field] = value;
        assert!(
            Replay::prepare(basis(), 3, 100)
                .unwrap()
                .verify(committed(&bytes, 1, 1000, 1001), &changed)
                .is_err(),
            "{field}"
        );
    }
    let mut wrong = basis();
    wrong.active_play_id = Some("play/foreign".into());
    assert!(Replay::prepare(wrong, 3, 100)
        .unwrap()
        .verify(committed(&bytes, 1, 1000, 1001), &valid)
        .is_err());
    assert!(Replay::prepare(basis(), 3, 100)
        .unwrap()
        .verify(committed(&bytes, 1, 1000, 999), &valid)
        .is_err());
    assert!(Replay::prepare(basis(), 3, 100)
        .unwrap()
        .verify(committed(&bytes, 1, 1000, 1001), &serde_json::Value::Null)
        .is_err());
    let mut replay = Replay::prepare(basis(), 3, 100).unwrap();
    replay
        .verify(committed(&bytes, 1, 1000, 1001), &valid)
        .unwrap();
    assert!(replay
        .verify(committed(&bytes, 1, 1001, 1002), &valid)
        .is_err());
    let mut changed = bytes.clone();
    changed.pop();
    assert!(replay
        .verify(committed(&changed, 1, 1000, 1002), &valid)
        .is_err());
    assert!(replay
        .verify(committed(&bytes, 3, 1002, 1003), &valid)
        .is_err());
}
