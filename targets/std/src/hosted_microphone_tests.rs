use super::*;
use conduit_core::{BaseImplementationId, BootId, HostId, OfferGeneration};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture(script: &str, suffix: &str) -> (PathBuf, PathBuf) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "conduit-alsa-microphone-{}-{suffix}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let executable = root.join("arecord");
    fs::write(&executable, script).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    (root, executable)
}

fn listing() -> &'static str {
    "**** List of CAPTURE Hardware Devices ****\ncard 1: DSP [SOF DSP], device 7: DMIC16kHz [DMIC16kHz]\n"
}

#[test]
fn parses_exact_capture_rows_and_empty_inventory() {
    let rows = parse_arecord_list(listing(), Path::new("/missing")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].card_id, "DSP");
    assert_eq!(rows[0].device, 7);
    assert_eq!(rows[0].base_identity, "alsa-card-1");
    assert!(parse_arecord_list("no devices\n", Path::new("/missing"))
        .unwrap()
        .is_empty());
}

#[test]
fn malformed_rows_and_invalid_limits_fail_closed() {
    assert_eq!(
        parse_arecord_list("card malformed\n", Path::new("/missing")),
        Err(MicrophoneFailure::InvalidDiscovery)
    );
    assert_eq!(
        validate_limits(MicrophoneLimits {
            capture_milliseconds: 0,
            timeout: Duration::from_secs(1),
        }),
        Err(MicrophoneFailure::InvalidLimits)
    );
}

#[test]
fn exact_discovery_selection_and_bounded_capture_produce_a_receipt() {
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = -l ]; then printf '{}'; exit 0; fi\ndd if=/dev/zero bs=32 count=1 2>/dev/null\nprintf diagnostic >&2\n",
        listing().replace('\n', "\\n")
    );
    let (root, executable) = fixture(&script, "success");
    let discovery = AlsaMicrophoneDiscovery::inspect(&executable).unwrap();
    assert_eq!(discovery.observations.len(), 1);
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    let raw = adapter.capture(|| false).unwrap();
    assert_eq!(raw, [0; 32]);
    let receipt = adapter.take_receipt().unwrap();
    assert_eq!(receipt.raw_pcm_bytes, 32);
    assert_eq!(receipt.diagnostic_bytes, 10);
    assert_eq!(receipt.card_id, "DSP");
    assert_eq!(receipt.device, 7);
    assert_eq!(receipt.sample_rate_hz, 16_000);
    assert!(!receipt.executable_sha256.is_empty());
    assert!(!receipt.raw_pcm_sha256.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_selection_and_oversized_capture_are_distinct() {
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = -l ]; then printf '{}'; exit 0; fi\ndd if=/dev/zero bs={} count=1 2>/dev/null\n",
        listing().replace('\n', "\\n"),
        MAXIMUM_RAW_PCM_BYTES + 1
    );
    let (root, executable) = fixture(&script, "overflow");
    let discovery = AlsaMicrophoneDiscovery::inspect(&executable).unwrap();
    let mut stale = discovery.observations[0].clone();
    stale.device = 8;
    assert!(matches!(
        discovery.clone().initialize(
            &stale,
            MicrophoneLimits {
                capture_milliseconds: 1_000,
                timeout: Duration::from_secs(2),
            }
        ),
        Err(MicrophoneFailure::SelectionDrift)
    ));
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 6_000,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    assert_eq!(
        adapter.capture(|| false),
        Err(MicrophoneFailure::OutputOverflow)
    );
    assert!(adapter.take_receipt().is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn discovery_overflow_and_short_capture_fail_closed() {
    let oversized = format!(
        "#!/bin/sh\nif [ \"$1\" = -l ]; then dd if=/dev/zero bs={} count=1 2>/dev/null; exit 0; fi\n",
        MAXIMUM_DISCOVERY_BYTES + 1
    );
    let (overflow_root, overflow_executable) = fixture(&oversized, "discovery-overflow");
    assert!(matches!(
        AlsaMicrophoneDiscovery::inspect(&overflow_executable),
        Err(MicrophoneFailure::DiscoveryFailed)
    ));
    fs::remove_dir_all(overflow_root).unwrap();

    let short = format!(
        "#!/bin/sh\nif [ \"$1\" = -l ]; then printf '{}'; exit 0; fi\nprintf '\\000\\000'\n",
        listing().replace('\n', "\\n")
    );
    let (short_root, short_executable) = fixture(&short, "short-capture");
    let discovery = AlsaMicrophoneDiscovery::inspect(&short_executable).unwrap();
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    assert_eq!(
        adapter.capture(|| false),
        Err(MicrophoneFailure::ShortCapture)
    );
    assert!(adapter.take_receipt().is_none());
    fs::remove_dir_all(short_root).unwrap();
}

#[test]
fn cancellation_kills_and_reaps_the_provider() {
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = -l ]; then printf '{}'; exit 0; fi\nwhile :; do :; done\n",
        listing().replace('\n', "\\n")
    );
    let (root, executable) = fixture(&script, "cancel");
    let discovery = AlsaMicrophoneDiscovery::inspect(&executable).unwrap();
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1_000,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    assert_eq!(adapter.capture(|| true), Err(MicrophoneFailure::Cancelled));
    assert!(adapter.take_receipt().is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn authorized_microphone_clip_runs_through_whisper_in_one_plan_play() {
    let microphone_script = format!(
        "#!/bin/sh\nif [ \"$1\" = -l ]; then printf '{}'; exit 0; fi\ndd if=/dev/zero bs=320 count=1 2>/dev/null\n",
        listing().replace('\n', "\\n")
    );
    let (microphone_root, microphone_executable) = fixture(&microphone_script, "plan-microphone");
    let discovery = AlsaMicrophoneDiscovery::inspect(&microphone_executable).unwrap();
    let selected = discovery.observations[0].clone();
    let microphone = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 10,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();

    let whisper_root =
        std::env::temp_dir().join(format!("conduit-microphone-whisper-{}", std::process::id()));
    let _ = fs::remove_dir_all(&whisper_root);
    fs::create_dir(&whisper_root).unwrap();
    let whisper_executable = whisper_root.join("whisper-cli");
    let whisper_model = whisper_root.join("model.bin");
    fs::write(
        &whisper_executable,
        "#!/bin/sh\nout=\nwhile [ $# -gt 0 ]; do if [ \"$1\" = --output-file ]; then out=$2; shift 2; else shift; fi; done\nprintf 'Rosehip House, hello\\n' > \"${out}.txt\"\n",
    )
    .unwrap();
    fs::set_permissions(&whisper_executable, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&whisper_model, b"bounded whisper fixture").unwrap();
    let whisper = crate::hosted_speech_recognition::WhisperDiscovery::inspect(
        &whisper_executable,
        &whisper_model,
    )
    .unwrap()
    .initialize(crate::hosted_speech_recognition::WhisperLimits {
        maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        threads: 1,
        timeout: Duration::from_secs(2),
    })
    .unwrap();

    let config = crate::StdHostConfig {
        host_id: HostId::from("microphone-plan-host"),
        boot_id: BootId::from("microphone-plan-boot"),
        offer_generation: OfferGeneration(1),
    };
    let mut host = crate::StdHost::new_with_microphone(
        config,
        crate::StdHostComposition::reference(),
        microphone,
    )
    .unwrap();
    host.attach_whisper_clip_recognizer(whisper).unwrap();

    let mut profiles = crate::installed_std::test_catalog();
    let mut startup = profiles.startup_catalog().unwrap();
    conduit_semantic_catalog::install_microphone_clip_catalogs(&mut startup, &mut profiles)
        .unwrap();
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profiles).unwrap();
    let checked = conduit_form::check_syntax_document(
        &conduit_form::parse_syntax_document(
            "form microphone-whisper {\n microphone: media/capture-microphone-clip\n recognize: speech/recognize-clip\n text: speech/recognition-to-text\n show: presentation/text\n \"capture\" > microphone.request\n microphone.clip > recognize.clip\n recognize.result > text.result\n text.text > show.text\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "microphone-whisper", &profiles).unwrap();
    let clip_connection = expanded
        .connections
        .iter()
        .find(|connection| connection.source_port_id.as_str() == "clip")
        .unwrap();
    let mut connection_limits = std::collections::BTreeMap::new();
    connection_limits.insert(
        (
            clip_connection.source_gear_id.clone(),
            clip_connection.source_port_id.clone(),
            clip_connection.sink_gear_id.clone(),
            clip_connection.sink_port_id.clone(),
        ),
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        },
    );
    let recognition_connection = expanded
        .connections
        .iter()
        .find(|connection| connection.source_port_id.as_str() == "result")
        .unwrap();
    connection_limits.insert(
        (
            recognition_connection.source_gear_id.clone(),
            recognition_connection.source_port_id.clone(),
            recognition_connection.sink_gear_id.clone(),
            recognition_connection.sink_port_id.clone(),
        ),
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
        },
    );
    let advertisements = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded, &advertisements).unwrap();
    let grant = host
        .microphone_authority_grant("grant/test-microphone-capture")
        .unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u32,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connection_limits,
    )
    .unwrap();
    let report = host
        .run_fragment_to(
            plan.fragments.into_iter().next().unwrap(),
            &mut Vec::new(),
            &mut crate::ThreadTimer,
        )
        .unwrap();
    assert_eq!(report.microphone.len(), 1);
    assert_eq!(report.microphone[0].raw_pcm_bytes, 320);
    assert_eq!(report.speech_recognition.len(), 1);
    assert!(report.speech_recognition[0].text_sha256.is_some());
    fs::remove_dir_all(microphone_root).unwrap();
    fs::remove_dir_all(whisper_root).unwrap();
}
