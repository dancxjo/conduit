use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn fixture(script: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "conduit-whisper-adapter-test-{}-{}",
        std::process::id(),
        NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let executable = root.join("whisper-cli");
    let model = root.join("model.bin");
    fs::write(&executable, script).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&model, b"bounded model").unwrap();
    (root, executable, model)
}

fn discover(executable: &Path, model: &Path) -> WhisperDiscovery {
    let discovery = WhisperDiscovery::inspect(executable, model).unwrap();
    let coverage = crate::hosted_language::tests::fixture_coverage(
        &discovery.provider_identity(),
        "en",
        "language/english",
    );
    discovery.declare_language_coverage(coverage).unwrap()
}
fn english() -> conduit_language::LanguageRequest {
    crate::hosted_language::tests::request("language/english")
}

fn pcm(samples: &[i16]) -> Vec<u8> {
    let payload = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect::<Vec<_>>();
    PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        16_000,
        PcmChannelLayout::Mono,
        samples.len() as u16,
        1,
        0,
        false,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap()
}

fn limits(timeout: Duration) -> WhisperLimits {
    WhisperLimits {
        maximum_audio_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
        maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        threads: 2,
        timeout,
    }
}

#[test]
fn exact_provider_emits_canonical_recognition_and_receipt() {
    let (root, executable, model) = fixture(
        r#"#!/bin/sh
out=
language=
while [ $# -gt 0 ]; do
  case "$1" in
    --output-file) out=$2; shift 2;;
    --language) language=$2; shift 2;;
    *) shift;;
  esac
done
[ "$language" = en ] || exit 17
printf 'Rosehip House, status\n' > "${out}.txt"
"#,
    );
    let discovery = discover(&executable, &model);
    let expected_model = discovery.model_sha256.clone();
    let mut adapter = discovery
        .initialize(limits(Duration::from_secs(2)))
        .unwrap();
    let evidence = adapter.enable_evidence_text();
    let audio = pcm(&[1, -2, 3, -4]);
    let encoded = adapter.recognize(&audio, &english(), || false).unwrap();
    let result = conduit_tongues::decode_speech_recognition_result(&encoded).unwrap();
    let conduit_tongues::SpeechRecognitionResult::Recognized(result) = result else {
        panic!("fixture provider did not produce recognized semantic info")
    };
    assert_eq!(result.text().get(), "Rosehip House, status");
    assert_eq!(evidence.bytes().unwrap(), b"Rosehip House, status");
    let receipt = adapter.take_receipt().unwrap();
    assert_eq!(receipt.audio_sha256, Sha256::digest(&audio).as_slice());
    assert_eq!(adapter.discovery().model_sha256, expected_model);
    drop(adapter);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn timeout_kills_provider_and_missing_output_is_failure() {
    let (root, executable, model) = fixture("#!/bin/sh\nsleep 2\n");
    let mut adapter = discover(&executable, &model)
        .initialize(limits(Duration::from_millis(20)))
        .unwrap();
    assert_eq!(
        adapter.recognize(&pcm(&[1]), &english(), || false),
        Err(WhisperFailure::Timeout)
    );
    drop(adapter);
    fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
    let mut adapter = discover(&executable, &model)
        .initialize(limits(Duration::from_secs(1)))
        .unwrap();
    assert_eq!(
        adapter.recognize(&pcm(&[1]), &english(), || false),
        Err(WhisperFailure::ReadFailed)
    );
    drop(adapter);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn profile_bounds_and_cancellation_fail_distinctly() {
    let (root, executable, model) = fixture("#!/bin/sh\nsleep 2\n");
    let mut adapter = discover(&executable, &model)
        .initialize(limits(Duration::from_secs(1)))
        .unwrap();
    assert_eq!(
        adapter.recognize(&pcm(&[1]), &english(), || true),
        Err(WhisperFailure::Cancelled)
    );
    let stereo = PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        16_000,
        PcmChannelLayout::StereoLeftRight,
        1,
        1,
        0,
        false,
    )
    .unwrap()
    .encode_frame(&[0, 0, 0, 0])
    .unwrap();
    assert_eq!(
        adapter.recognize(&stereo, &english(), || false),
        Err(WhisperFailure::UnsupportedPcmProfile)
    );
    drop(adapter);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bare_discovery_and_wrong_language_never_start_the_provider() {
    let (root, executable, model) = fixture(
        r#"#!/bin/sh
touch "$(dirname "$0")/provider-started"
exit 0
"#,
    );
    let bare = WhisperDiscovery::inspect(&executable, &model).unwrap();
    let mut adapter = bare.initialize(limits(Duration::from_secs(1))).unwrap();
    assert!(adapter.offer().realization_properties.is_empty());
    assert_eq!(
        adapter.recognize(&pcm(&[1]), &english(), || false),
        Err(WhisperFailure::Language(HostedLanguageRefusal::Coverage(
            conduit_language::LanguageCoverageRefusal::Undeclared
        )))
    );
    drop(adapter);
    let mut adapter = discover(&executable, &model)
        .initialize(limits(Duration::from_secs(1)))
        .unwrap();
    let french = crate::hosted_language::tests::request("language/french");
    assert_eq!(
        adapter.recognize(&pcm(&[1]), &french, || false),
        Err(WhisperFailure::Language(HostedLanguageRefusal::Coverage(
            conduit_language::LanguageCoverageRefusal::Language
        )))
    );
    assert!(!root.join("provider-started").exists());
    drop(adapter);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn another_artifact_cannot_lend_coverage_and_source_drift_is_distinct() {
    let (root, executable, model) = fixture("#!/bin/sh\nexit 0\n");
    let discovery = WhisperDiscovery::inspect(&executable, &model).unwrap();
    let foreign = crate::hosted_language::tests::fixture_coverage(
        "whisper/provider/another-source",
        "en",
        "language/english",
    );
    assert_eq!(
        discovery.clone().declare_language_coverage(foreign),
        Err(WhisperFailure::Language(HostedLanguageRefusal::Artifact))
    );
    let mut adapter = discover(&executable, &model)
        .initialize(limits(Duration::from_secs(1)))
        .unwrap();
    fs::write(&model, b"changed model").unwrap();
    assert_eq!(
        adapter.recognize(&pcm(&[1]), &english(), || false),
        Err(WhisperFailure::ProviderChanged)
    );
    drop(adapter);
    fs::remove_dir_all(root).unwrap();
}
