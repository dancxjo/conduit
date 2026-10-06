//! Explicit installed-engine and physical speaker proofs.
use super::*;

/// Explicit local device proof. The caller must name an exact currently
/// discovered card and device; no default or first-device fallback is allowed.
#[test]
#[ignore = "requires installed eSpeak NG, explicit ALSA card/device, and speaker output"]
fn installed_espeak_stream_drains_selected_alsa_speaker() {
    let card = std::env::var("CONDUIT_SPOKEN_TEST_ALSA_CARD").unwrap();
    let device = std::env::var("CONDUIT_SPOKEN_TEST_ALSA_DEVICE")
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let observed = crate::hosted_audio::discover_alsa_playback()
        .unwrap()
        .into_iter()
        .find(|item| item.card_id == card && item.device == device)
        .expect("explicitly selected speaker is absent from current discovery");
    let config = StdHostConfig {
        host_id: HostId::from("local-spoken-playback-host"),
        boot_id: BootId::from(format!("local-spoken-playback-boot-{}", std::process::id())),
        offer_generation: OfferGeneration(1),
    };
    let selected = HostedPlaybackSelection::from_observation(
        observed,
        config.boot_id.clone(),
        config.offer_generation,
    );
    let discovery = EspeakDiscovery::inspect(
        Path::new("/usr/bin/espeak-ng"),
        Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
        "en-us",
        &[PathBuf::from(
            "/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1.1.51",
        )],
    )
    .unwrap();
    let discovery = declare_english_fixture(discovery);
    let (face, show) = source(2);
    let (_, batch) = batch(&face, &show);
    let authorization = ExplicitPlaybackAuthorization::new("grant/local-spoken-speaker").unwrap();
    let result = execute_real_spoken_batch_to_selected_playback(
        &face,
        &show,
        &batch,
        discovery,
        &crate::hosted_language::tests::request("language/english"),
        config,
        selected,
        &authorization,
        &crate::RunControl::default(),
    )
    .unwrap();
    assert_eq!(result.outcome, SpokenPlaybackOutcome::Completed);
    assert_eq!(result.playback.lifecycle, PlaybackLifecycle::StoppedClosed);
    assert!(result.playback.metrics.blocks_committed > 1);
    assert_eq!(result.source_segments_sha256, batch.source_segments_sha256);
    eprintln!("selected spoken playback receipt: {result:?}");
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
    let discovery = declare_english_fixture(discovery);
    let directory =
        std::env::temp_dir().join(format!("conduit-spoken-face-real-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let wav = directory.join("speech.wav");
    let result = execute_real_spoken_batch(
        &face,
        &show,
        &batch,
        discovery,
        &crate::hosted_language::tests::request("language/english"),
        &wav,
    )
    .unwrap();
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

fn declare_english_fixture(discovery: EspeakDiscovery) -> EspeakDiscovery {
    let coverage = crate::hosted_language::tests::fixture_coverage(
        &discovery.provider_identity(),
        "en-us",
        "language/english",
    );
    discovery.declare_language_coverage(coverage).unwrap()
}
