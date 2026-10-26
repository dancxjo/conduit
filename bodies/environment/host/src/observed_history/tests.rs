use super::*;
use ed25519_dalek::{Signer, SigningKey};
fn signed(
    key: &SigningKey,
    scope: [u8; 96],
    forms: [u8; 32],
    after: u32,
    committed: u32,
    records: &[u8],
) -> Vec<u8> {
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    serde_json::to_vec(&serde_json::json!({"profile":"observed-history@1","scope":hex(&scope),"forms":hex(&forms),"after":after,"committed":committed,"records":hex(records),"signature":hex(&key.sign(&wire::transcript(&scope,&forms,after,committed,records)).to_bytes())})).unwrap()
}
#[test]
fn replay_requires_exact_scope_key_cursor_forms_and_continuous_records() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let collector = Collector {
        scope: [3; 96],
        forms: [4; 32],
        key: key.verifying_key(),
        after: 0,
        observed_ms: 0,
    };
    let record = ObservedRecord {
        sequence: 1,
        observed_at_boot_ms: 123,
        disposition: conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
        quantities: None,
    }
    .encode()
    .unwrap();
    let good = signed(&key, collector.scope, collector.forms, 0, 1, &record);
    assert_eq!(collector.validate(&good).unwrap(), (1, 123));
    assert!(collector
        .validate(&signed(&key, [8; 96], collector.forms, 0, 1, &record))
        .is_err());
    assert!(collector
        .validate(&signed(&key, collector.scope, [8; 32], 0, 1, &record))
        .is_err());
    assert!(collector
        .validate(&signed(
            &SigningKey::from_bytes(&[8; 32]),
            collector.scope,
            collector.forms,
            0,
            1,
            &record
        ))
        .is_err());
    assert!(collector
        .validate(&signed(&key, collector.scope, collector.forms, 1, 1, &[]))
        .is_err());
    assert!(collector
        .validate(&signed(&key, collector.scope, collector.forms, 0, 1, &[]))
        .is_err());
    let mut skipped = record;
    skipped[..8].copy_from_slice(&2u64.to_le_bytes());
    assert!(collector
        .validate(&signed(
            &key,
            collector.scope,
            collector.forms,
            0,
            1,
            &skipped
        ))
        .is_err());
    let mut altered: serde_json::Value = serde_json::from_slice(&good).unwrap();
    altered["committed"] = 2.into();
    assert!(collector
        .validate(&serde_json::to_vec(&altered).unwrap())
        .is_err());
    let later = Collector {
        after: 1,
        observed_ms: 200,
        ..collector
    };
    assert!(later
        .validate(&signed(&key, later.scope, later.forms, 1, 2, &skipped))
        .is_err());
}

#[test]
fn verified_history_fills_face_gap_without_changing_observation_clock() {
    use conduit_environment_runtime::{
        observation::{ObservationContract, PreparedSourceFrameDecoder},
        publication::{PublicationChannel, PublicationSnapshot},
        recording::RecordingDisposition,
        retained_publication_face::RetainedPublicationFace,
    };
    let key = SigningKey::from_bytes(&[19; 32]);
    let collector = Collector {
        scope: [3; 96],
        forms: [4; 32],
        key: key.verifying_key(),
        after: 3,
        observed_ms: 250,
    };
    let record = |sequence, observed_at_boot_ms| ObservedRecord {
        sequence,
        observed_at_boot_ms,
        disposition: RecordingDisposition::Available,
        quantities: Some((
            conduit_core::Quantity::new(20, conduit_core::Unit::Celsius),
            conduit_core::Quantity::new(59, conduit_core::Unit::Percent),
        )),
    };
    let mut face = RetainedPublicationFace::prepare(
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("fixture/source".into()),
            checked_plot_id: Some("fixture/checked".into()),
            expanded_plot_id: Some("fixture/expanded".into()),
            plan_id: Some("fixture/plan".into()),
            active_play_id: Some("fixture/play".into()),
            sign_ids: vec![],
        },
        960,
        120000,
    )
    .unwrap();
    face.publish_projected(&record(1, 100), 100).unwrap();
    face.at(170).unwrap();
    let directory =
        std::env::temp_dir().join(format!("conduit-history-gap-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let records = [
        record(1, 100).encode().unwrap(),
        record(2, 160).encode().unwrap(),
        record(3, 250).encode().unwrap(),
    ]
    .concat();
    std::fs::write(
        directory.join("observed-history-0000-0003.json"),
        signed(&key, collector.scope, collector.forms, 0, 3, &records),
    )
    .unwrap();
    collector.replay_gap(&directory, &mut face, 3, 300).unwrap();
    assert_eq!(face.last_source_sequence(), 2);
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    let canonical = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
    face.publish_observed(
        PublicationSnapshot {
            sequence: 3,
            channel: PublicationChannel::Reading,
            canonical,
        },
        250,
        300,
    )
    .unwrap();
    assert_eq!(face.last_source_sequence(), 3);
    let observed = face
        .current()
        .properties
        .iter()
        .find(|p| p.name == "observed-at-boot-ms")
        .unwrap();
    assert_eq!(observed.value, conduit_face::FacePropertyValue::Count(250));
    assert!(face.publish_projected(&record(4, 400), 350).is_err());
    assert_eq!(face.last_source_sequence(), 3);
    std::fs::remove_file(directory.join("observed-history-0000-0003.json")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
