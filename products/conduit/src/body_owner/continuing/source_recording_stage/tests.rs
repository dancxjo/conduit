use super::*;
use conduit_environment_measurement::source_observation::{
    PreparedObservationCodec, SourceFailure, SourceObservation, SourceOutcome,
};
fn selection() -> Arc<SourceSelection> {
    Arc::new(SourceSelection {
        attachment_id: "green".into(),
        host_id: "host/stage".into(),
        boot_id: "boot/stage".into(),
        capability_id: "cap/source".into(),
        provider_instance_id: "provider/source".into(),
        provider_generation: 1,
        plan_id: "plan/stage".into(),
        fragment_id: "fragment/stage".into(),
        placement_id: "placement/source".into(),
        configuration_id: "configuration/source".into(),
        firmware_sha256: [7; 32],
    })
}
fn fore(selection: &Arc<SourceSelection>, sequence: u64, play: u64, time: u64) -> LiveFore {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let value = codec
        .encode(SourceObservation {
            observed_at_host_boot_ms: time,
            outcome: SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
        })
        .unwrap();
    let mut bytes = [0; MAXIMUM_BYTES];
    bytes[..value.len()].copy_from_slice(value);
    LiveFore {
        scope: Arc::new(CompletedScope {
            selection: Arc::clone(selection),
            native_play: conduit_core::bind_active_play(
                &selection.plan_id,
                &selection.host_id,
                &selection.boot_id,
                play,
            ),
        }),
        sequence,
        length: value.len(),
        bytes,
    }
}
#[test]
fn staged_source_fore_is_bounded_ordered_and_duplicate_idempotent() {
    let selection = selection();
    let body: conduit_body::BodyId = body_id(1);
    let mut stage = SourceRecordingStage::prepare(body.clone(), Arc::clone(&selection)).unwrap();
    stage.check_selection(&body, &selection).unwrap();
    assert!(stage.check_selection(&body_id(2), &selection).is_err());
    assert_eq!(
        stage.validate(&fore(&selection, 1, 1, 10)),
        Err(Refusal::Sequence)
    );
    assert_eq!(stage.len(), 0);
    assert!(stage.scope().is_none());
    for i in 0..CAPACITY {
        let value = fore(&selection, i as u64, 1, 10 + i as u64);
        assert_eq!(stage.validate(&value), Ok(Accepted::Added));
        assert_eq!(stage.accept(&value), Ok(Accepted::Added));
        assert_eq!(stage.accept(&value), Ok(Accepted::Duplicate));
    }
    let before: Vec<_> = stage
        .observations()
        .map(|r| (r.sequence(), r.canonical().to_vec()))
        .collect();
    assert_eq!(
        stage.accept(&fore(&selection, 16, 1, 26)),
        Err(Refusal::Capacity)
    );
    assert_eq!(
        stage.accept(&fore(&selection, 0, 1, 999)),
        Err(Refusal::Conflict)
    );
    assert_eq!(
        stage.accept(&fore(&selection, 0, 2, 10)),
        Err(Refusal::Scope)
    );
    assert_eq!(
        stage
            .observations()
            .map(|r| (r.sequence(), r.canonical().to_vec()))
            .collect::<Vec<_>>(),
        before
    );
}
#[test]
fn staged_source_fore_invalid_scope_or_bytes_do_not_adopt_play() {
    let selection = selection();
    let mut stage = SourceRecordingStage::prepare(body_id(1), Arc::clone(&selection)).unwrap();
    let mut invalid = fore(&selection, 0, 1, 10);
    invalid.length = MAXIMUM_BYTES + 1;
    assert_eq!(stage.accept(&invalid), Err(Refusal::Observation));
    invalid.length = 1;
    invalid.bytes[0] = 255;
    assert_eq!(stage.accept(&invalid), Err(Refusal::Observation));
    let mut other = selection.as_ref().clone();
    other.provider_generation = 2;
    assert_eq!(
        stage.accept(&fore(&Arc::new(other), 0, 1, 10)),
        Err(Refusal::Scope)
    );
    assert!(stage.scope().is_none());
    assert_eq!(stage.len(), 0);
    assert_eq!(
        stage.accept(&fore(&selection, 0, 7, 10)),
        Ok(Accepted::Added)
    );
    assert_eq!(stage.scope().unwrap().native_play.play_sequence, 7);
}

fn body_id(birth: u64) -> conduit_body::BodyId {
    conduit_body::Body::born(
        "source/stage".into(),
        "plot/stage".into(),
        birth,
        "sign/born".into(),
    )
    .unwrap()
    .body_id
}

#[test]
fn staged_source_checkpoint_preparation_preserves_all_bytes_and_stable_identity() {
    let mut selected = selection().as_ref().clone();
    selected.attachment_id = "a".repeat(220);
    selected.host_id = "h".repeat(220).into();
    selected.boot_id = "b".repeat(220).into();
    selected.capability_id = "c".repeat(220).into();
    selected.provider_instance_id = "p".repeat(220).into();
    selected.plan_id = "q".repeat(220).into();
    selected.fragment_id = "f".repeat(220).into();
    selected.placement_id = "l".repeat(220).into();
    selected.configuration_id = "g".repeat(220);
    let selected = Arc::new(selected);
    let body = body_id(1);
    let mut stage = SourceRecordingStage::prepare(body.clone(), selected.clone()).unwrap();
    for sequence in 0..CAPACITY {
        stage
            .accept(&fore(&selected, sequence as u64, 9, sequence as u64))
            .unwrap();
    }
    let original: Vec<_> = stage
        .observations()
        .map(|v| v.canonical().to_vec())
        .collect();
    stage.prepare_checkpoint_input().unwrap();
    let prepared = stage.prepared.as_ref().unwrap();
    let content = prepared.snapshot.content().unwrap().to_vec();
    let reference = prepared.reference.clone();
    let recovered = conduit_environment_runtime::source_recording::SourceRecordingSnapshot::read(
        &content,
        prepared.snapshot.context(),
    )
    .unwrap();
    assert!(recovered.len() > 0 && recovered.len() < CAPACITY);
    assert_eq!(recovered.context().body_id, body.as_str());
    assert_eq!(recovered.context().native_play.play_sequence, 9);
    for index in 0..recovered.len() {
        let (sequence, bytes) = recovered.observation(index).unwrap();
        assert_eq!(sequence, index as u64 + 1);
        assert_eq!(bytes, original[index]);
    }
    assert!(content.len() <= 4096);
    assert_eq!(reference.extent.bytes, content.len() as u64);
    assert_eq!(stage.len(), CAPACITY);
    stage.prepare_checkpoint_input().unwrap();
    assert_eq!(stage.prepared.as_ref().unwrap().reference, reference);
    assert_eq!(
        stage.prepared.as_ref().unwrap().snapshot.content().unwrap(),
        content
    );
    assert_eq!(
        stage
            .observations()
            .map(|v| v.canonical().to_vec())
            .collect::<Vec<_>>(),
        original
    );
    assert_eq!(
        stage.prepared_input_metadata().unwrap()["producer_retirement_authorized"],
        false
    );
}
