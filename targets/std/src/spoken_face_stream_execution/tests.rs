use super::*;
use crate::spoken_face_mask::{
    ReaderCommand, SpokenBatchDelivery, SpokenFaceRefusal, SpokenFaceSession, SpokenTurnOutcome,
};
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

/// Installed eSpeak is a platform proof, kept explicit and run only where its
/// trusted executable, data, and engine are present. It retains the WAV in a
/// fresh temporary directory so the returned bytes can be inspected.
#[test]
#[ignore = "requires installed eSpeak NG and writes a real WAV"]
fn installed_espeak_produces_exact_plan_play_pcm_receipt() {
    let (face, show) = source(2);
    let (mut reader, batch) = batch(&face, &show);
    let discovery = EspeakDiscovery::inspect(
        Path::new("/usr/bin/espeak-ng"),
        Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
        "en-us",
        &[PathBuf::from(
            "/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1.1.51",
        )],
    )
    .unwrap();
    let directory =
        std::env::temp_dir().join(format!("conduit-spoken-face-real-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let wav = directory.join("speech.wav");
    let result = execute_real_spoken_batch(&face, &show, &batch, discovery, &wav).unwrap();
    assert!(result.receipt.pcm_bytes > 0);
    assert!(result.receipt.pcm_blocks > 0);
    assert_eq!(result.receipt.wav_bytes, fs::metadata(&wav).unwrap().len());
    let produced = fs::read(&wav).unwrap();
    assert!(produced[44..].iter().any(|sample| *sample != 0));
    assert_eq!(
        result.receipt.wav_sha256,
        format!("{:x}", Sha256::digest(&produced))
    );
    assert_eq!(
        result.receipt.source_segments_sha256,
        batch.source_segments_sha256
    );
    let sources = serde_json::json!({
        "faceId": batch.face_id,
        "faceRevision": batch.face_revision,
        "sourceShowId": batch.source_show_id,
        "streamIdentity": batch.stream_identity,
        "sourceSegmentsSha256": batch.source_segments_sha256,
        "segments": batch.segments.iter().map(|item| serde_json::json!({
            "sequence": item.segment.sequence,
            "text": item.segment.text,
            "textSha256": item.text_sha256,
            "reason": item.segment.reason,
        })).collect::<Vec<_>>(),
    });
    fs::write(
        directory.join("source-segments.json"),
        serde_json::to_vec_pretty(&sources).unwrap(),
    )
    .unwrap();
    let receipt = &result.receipt;
    let produced_receipt = serde_json::json!({
        "sourceSegmentsSha256": receipt.source_segments_sha256,
        "streamIdentity": receipt.stream_identity,
        "sourceShowId": receipt.source_show_id,
        "speechPlanId": receipt.speech_plan_id,
        "speechPlayId": receipt.speech_play_id,
        "providerSha256": receipt.provider_sha256,
        "wavSha256": receipt.wav_sha256,
        "wavBytes": receipt.wav_bytes,
        "pcmBytes": receipt.pcm_bytes,
        "pcmBlocks": receipt.pcm_blocks,
    });
    fs::write(
        directory.join("speech-receipt.json"),
        serde_json::to_vec_pretty(&produced_receipt).unwrap(),
    )
    .unwrap();
    let terminal = reader
        .acknowledge_batch(SpokenBatchDelivery::Completed(result.receipt))
        .unwrap()
        .unwrap();
    assert_eq!(terminal.outcome, SpokenTurnOutcome::Completed);
    assert_eq!(terminal.completed_segments, 2);
    eprintln!("real spoken Face WAV: {}", wav.display());
}
