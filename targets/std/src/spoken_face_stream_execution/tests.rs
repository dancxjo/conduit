use super::*;
use crate::hosted_audio::{
    AlsaPlaybackObservation, ExplicitPlaybackAuthorization, FakePlaybackBehavior,
    HostedPlaybackSelection, PlaybackLifecycle,
};
use crate::spoken_face_mask::{
    ReaderCommand, SpokenBatchDelivery, SpokenFaceRefusal, SpokenFaceSession, SpokenTurnOutcome,
};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_presentation::{PresentationRole, PresentationSubject};

// Both spoken contracts share the one established presentation fixture source.
#[allow(clippy::duplicate_mod)]
#[path = "../../../../semantics/presentation/tests/common/mod.rs"]
mod common;

fn source(subjects: usize) -> (Presentation, MaskShow) {
    let plot = common::checked_renderer_plot();
    let plan = common::plan_for(
        &plot,
        common::host(
            "linux-host",
            "linux-boot",
            "renderer-wayland",
            "presentation/renderer-wayland@1",
            "patchbay-native/wayland@1",
            "presentation/base/wayland-surface@1",
            common::WAYLAND_RESOURCE,
        ),
    );
    let base = common::presentation(&plot, &plan);
    let mut basis = base.basis;
    basis.body_id = None;
    basis.wake_id = None;
    let subjects = (0..subjects)
        .map(|index| PresentationSubject {
            identity: if index == 0 {
                "patchbay/plot".into()
            } else {
                format!("item/{index}")
            },
            role: PresentationRole::Region,
            name: if index == 0 {
                "Welcome".into()
            } else {
                format!("Item {index}")
            },
        })
        .collect();
    let face = Presentation::new_with_semantics(
        1,
        basis,
        subjects,
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    (face, show)
}

fn batch(face: &Presentation, show: &MaskShow) -> (SpokenFaceSession, SpokenBatch) {
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(face, show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let batch = reader.next_batch().unwrap().unwrap();
    (reader, batch)
}

#[test]
fn exact_single_segment_is_admitted_and_receipt_is_terminal_only_after_audio() {
    let (face, show) = source(1);
    let (mut reader, batch) = batch(&face, &show);
    assert_eq!(batch.segments.len(), 1);
    assert_eq!(batch.segments[0].segment.sequence, 0);
    assert_eq!(batch.encoded_inputs().unwrap().len(), 1);
    let destination =
        std::env::temp_dir().join(format!("conduit-spoken-face-{}.wav", std::process::id()));
    assert_eq!(
        validate_batch_for_installed_fore(&face, &show, &batch, &destination),
        Ok(())
    );
    assert_eq!(reader.next_batch(), Err(SpokenFaceRefusal::SpeechPressure));
    let mut stale = face.clone();
    stale.revision += 1;
    assert_eq!(
        validate_batch_for_installed_fore(&stale, &show, &batch, &destination),
        Err(SpokenStreamExecutionRefusal::StaleFace)
    );
    assert_eq!(
        reader
            .acknowledge_batch(SpokenBatchDelivery::Cancelled)
            .unwrap()
            .unwrap()
            .outcome,
        SpokenTurnOutcome::Cancelled
    );
}

#[test]
fn multiple_real_committed_segments_pass_preflight_without_creating_artifact() {
    let (face, show) = source(2);
    let (_, batch) = batch(&face, &show);
    assert_eq!(batch.segments.len(), 2);
    assert_eq!(batch.segments[0].segment.sequence, 0);
    assert_eq!(batch.segments[1].segment.sequence, 1);
    let destination = std::env::temp_dir().join(format!(
        "conduit-spoken-face-refused-{}.wav",
        std::process::id()
    ));
    assert_eq!(
        validate_batch_for_installed_fore(&face, &show, &batch, &destination),
        Ok(())
    );
    assert!(!destination.exists());
}

fn selected_fake_playback(
    behavior: FakePlaybackBehavior,
) -> (
    StdHostConfig,
    HostedPlaybackSelection,
    ExplicitPlaybackAuthorization,
) {
    let config = StdHostConfig {
        host_id: HostId::from("spoken-playback-fixture-host"),
        boot_id: BootId::from("spoken-playback-fixture-boot"),
        offer_generation: OfferGeneration(4),
    };
    let selection = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 3,
            card_id: "SPEECH_TEST".into(),
            card_name: "Selected fake speaker".into(),
            device: 2,
            device_name: "Finite PCM sink".into(),
            base_identity: "fixture-speaker".into(),
        },
        config.boot_id.clone(),
        config.offer_generation,
        behavior,
    )
    .with_bounded_speech_queue();
    let authorization = ExplicitPlaybackAuthorization::new("grant/spoken-test-speaker").unwrap();
    (config, selection, authorization)
}

#[test]
fn selected_playback_block_admission_covers_its_declared_duration() {
    let blocks = 32_768_u64;
    let millis = 30_000_u64;
    let source_frames_per_block = u64::from(conduit_std_offers::SPEECH_FRAMES_PER_BLOCK);
    let required = (millis * 22_050).div_ceil(source_frames_per_block * 1_000)
        + u64::try_from(conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS).unwrap();
    assert!(
        3_072 < required,
        "the former admission was shorter than its duration"
    );
    assert!(blocks >= required);
    assert!(blocks <= u64::from(conduit_semantic_catalog::AUDIO_STREAM_MAXIMUM_BLOCKS));
    assert!(blocks <= u64::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS));
    assert_eq!(
        spoken_playback_plot(&crate::hosted_language::tests::request("language/english"))
            .matches("maximum-blocks = 32768, maximum-audio-millis = 30000")
            .count(),
        2,
        "converter and speaker must admit the full declared duration"
    );
}

fn deterministic_playback(
    behavior: FakePlaybackBehavior,
    control: &crate::RunControl,
) -> Result<(SpokenFaceSession, SpokenPlaybackExecution), SpokenStreamExecutionRefusal> {
    // The deterministic proof synthesizer restarts its PCM clock for each
    // separate text segment; the real stream provider keeps a continuous one.
    let (face, show) = source(1);
    let (reader, batch) = batch(&face, &show);
    let (config, selection, authorization) = selected_fake_playback(behavior);
    let mut host = StdHost::new_with_playback(
        config.clone(),
        StdHostComposition::minimal().with_text(),
        selection.clone(),
    )
    .unwrap();
    let execution = super::playback::run_selected_spoken_playback(
        &face,
        &show,
        &batch,
        &crate::hosted_language::tests::request("language/english"),
        &"00".repeat(32),
        config,
        selection,
        &authorization,
        control,
        &mut host,
        false,
    )?;
    Ok((reader, execution))
}

#[test]
fn selected_playback_keeps_the_same_host_across_two_plays() {
    let (face, show) = source(1);
    let (_, batch) = batch(&face, &show);
    let (config, selection, authorization) = selected_fake_playback(FakePlaybackBehavior::Success);
    let mut host = StdHost::new_with_playback(
        config.clone(),
        StdHostComposition::minimal().with_text(),
        selection.clone(),
    )
    .unwrap();
    let original = host.advertisement().clone();
    let first = super::playback::run_selected_spoken_playback(
        &face,
        &show,
        &batch,
        &crate::hosted_language::tests::request("language/english"),
        &"00".repeat(32),
        config.clone(),
        selection.clone(),
        &authorization,
        &crate::RunControl::default(),
        &mut host,
        false,
    )
    .unwrap();
    let second = super::playback::run_selected_spoken_playback(
        &face,
        &show,
        &batch,
        &crate::hosted_language::tests::request("language/english"),
        &"00".repeat(32),
        config,
        selection,
        &authorization,
        &crate::RunControl::default(),
        &mut host,
        false,
    )
    .unwrap();
    assert_eq!(host.advertisement(), &original);
    assert_eq!(first.outcome, SpokenPlaybackOutcome::Completed);
    assert_eq!(second.outcome, SpokenPlaybackOutcome::Completed);
    assert_ne!(first.playback_play_id, second.playback_play_id);
}

#[test]
fn attached_host_entrance_refuses_a_missing_speech_provider() {
    let (face, show) = source(1);
    let (_, batch) = batch(&face, &show);
    let (config, selection, authorization) = selected_fake_playback(FakePlaybackBehavior::Success);
    let mut host = StdHost::new_with_playback(
        config,
        StdHostComposition::minimal().with_text(),
        selection.clone(),
    )
    .unwrap();
    let before = host.advertisement().clone();
    let refusal = super::playback::execute_spoken_batch_on_attached_host(
        &face,
        &show,
        &batch,
        &crate::hosted_language::tests::request("language/english"),
        &selection,
        &authorization,
        &crate::RunControl::default(),
        &mut host,
        false,
    )
    .unwrap_err();
    assert!(matches!(refusal, SpokenStreamExecutionRefusal::Plan(_)));
    assert_eq!(host.advertisement(), &before);
}

#[test]
fn selected_speaker_play_is_distinct_from_wav_and_drains_ordered_segments() {
    let (mut reader, result) =
        deterministic_playback(FakePlaybackBehavior::Success, &crate::RunControl::default())
            .unwrap();
    assert_eq!(result.outcome, SpokenPlaybackOutcome::Completed);
    assert_eq!(result.playback.lifecycle, PlaybackLifecycle::StoppedClosed);
    assert!(result.playback.metrics.blocks_committed > 1);
    assert_eq!(result.playback.metrics.underruns, 0);
    assert_eq!(
        result.playback.backend,
        "deterministic-bounded-speech-fixture@1"
    );
    assert!(result.selected_resource_pool_id.contains("fixture-speaker"));
    assert_eq!(result.authority_grant_id, "grant/spoken-test-speaker");
    assert!(!result.playback_plan_id.is_empty());
    assert!(!result.playback_play_id.is_empty());
    assert!(!result.source_segments_sha256.is_empty());
    let finished = reader
        .acknowledge_batch(result.delivery())
        .unwrap()
        .unwrap();
    assert_eq!(finished.outcome, SpokenTurnOutcome::Completed);
    assert_eq!(finished.completed_segments, 1);
}

#[test]
fn spoken_turn_rejects_undrained_speaker_receipt_without_losing_pending_batch() {
    let (mut reader, result) =
        deterministic_playback(FakePlaybackBehavior::Success, &crate::RunControl::default())
            .unwrap();
    let mut incomplete = result.delivery();
    let SpokenBatchDelivery::Played(receipt) = &mut incomplete else {
        panic!("completed speaker Play must have a playback receipt");
    };
    receipt.playback.lifecycle = PlaybackLifecycle::Active;
    assert_eq!(
        reader.acknowledge_batch(incomplete),
        Err(SpokenFaceRefusal::SpeechReceipt)
    );
    assert_eq!(
        reader
            .acknowledge_batch(result.delivery())
            .unwrap()
            .unwrap()
            .outcome,
        SpokenTurnOutcome::Completed
    );
}

#[test]
fn stopped_spoken_play_does_not_report_completed_playback() {
    let control = crate::RunControl::default();
    control
        .request_stop(crate::RunControlRequestId::new("stop/spoken-before-open").unwrap())
        .unwrap();
    let (mut reader, result) =
        deterministic_playback(FakePlaybackBehavior::Success, &control).unwrap();
    assert_eq!(result.outcome, SpokenPlaybackOutcome::Cancelled);
    assert_eq!(result.playback.metrics.blocks_committed, 0);
    assert_eq!(
        reader
            .acknowledge_batch(result.delivery())
            .unwrap()
            .unwrap()
            .outcome,
        SpokenTurnOutcome::Cancelled
    );
}

#[test]
fn selected_speaker_loss_is_not_a_completed_spoken_show() {
    let error = deterministic_playback(
        FakePlaybackBehavior::ProviderLossAfterFirstBlock,
        &crate::RunControl::default(),
    )
    .err()
    .expect("speaker loss must refuse the Play");
    assert!(
        matches!(error, SpokenStreamExecutionRefusal::PlaybackPlay { detail, source_show_id, source_segments_sha256, plan_id, .. }
        if detail.contains("HostCallFailed") && detail.contains("detail: 75")
            && !source_show_id.is_empty() && !source_segments_sha256.is_empty() && !plan_id.is_empty())
    );
}

#[path = "tests/platform.rs"]
mod platform;

#[path = "tests/language.rs"]
mod language;
