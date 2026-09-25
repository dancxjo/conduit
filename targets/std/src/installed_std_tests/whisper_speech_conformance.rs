use super::RecordingTimer;
use crate::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_core::{
    BaseImplementationId, BootId, HostId, ObservationKind, OfferGeneration, TerminalDisposition,
};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

#[test]
fn initialized_whisper_runs_portable_recognition_through_ordinary_plan_and_play() {
    let root =
        std::env::temp_dir().join(format!("conduit-whisper-plan-play-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let executable = root.join("whisper-cli");
    let model = root.join("model.bin");
    fs::write(
        &executable,
        "#!/bin/sh\nout=\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = --output-file ]; then out=$2; shift 2; else shift; fi\ndone\nprintf 'Rosehip House, status\\n' > \"${out}.txt\"\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&model, b"bounded whisper model fixture").unwrap();
    let adapter = WhisperDiscovery::inspect(&executable, &model)
        .unwrap()
        .initialize(WhisperLimits {
            maximum_audio_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
            maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
            threads: 2,
            timeout: Duration::from_secs(2),
        })
        .unwrap();
    let mut host = StdHost::new_with_whisper_speech_recognition(
        StdHostConfig {
            host_id: HostId::from("whisper-plan-play-host"),
            boot_id: BootId::from("whisper-plan-play-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        adapter,
    )
    .unwrap();
    assert!(host.advertisement().resources.iter().any(|resource| {
        resource.class_id.as_str() == conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS
    }));

    let mut catalog = crate::installed_std::test_catalog();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut catalog).unwrap();
    crate::installed_std::test_local_model_io::install_house_source_catalog(
        &mut startup,
        &mut catalog,
    );
    crate::installed_std::test_local_model_io::install_house_text_sink_catalog(
        &mut startup,
        &mut catalog,
    );
    let source = format!(
        "form whisper_proof {{\n audio: {}\n recognize: speech/recognize\n text: speech/recognition-to-text\n sink: {}\n audio.value >> recognize.audio\n recognize.result >> text.result\n text.text >> sink.value\n}}\n",
        crate::installed_std::test_local_model_io::HOUSE_AUDIO_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_TEXT_SINK_KIND,
    );
    let form = conduit_form::parse(&source, &catalog).unwrap();
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &advertisements).unwrap();
    let plan = conduit_planner::plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::RECOGNITION_RESULT_QUEUE_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.implementation_id.as_str()
                == conduit_std_offers::WHISPER_SPEECH_IMPLEMENTATION
        })
        .unwrap();
    assert_eq!(placement.resources.len(), 1);
    let report = host
        .run_fragment_to(
            plan.fragments[0].clone(),
            &mut Vec::with_capacity(1_024),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert_eq!(report.speech_recognition.len(), 1);
    let receipt = &report.speech_recognition[0];
    assert_eq!(
        receipt.implementation_id.as_str(),
        conduit_std_offers::WHISPER_SPEECH_IMPLEMENTATION
    );
    assert_eq!(receipt.text_bytes, 21);
    assert!(receipt.text_sha256.is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn initialized_whisper_assembles_a_bounded_clip_in_one_ordinary_play() {
    let root = std::env::temp_dir().join(format!(
        "conduit-whisper-clip-plan-play-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let executable = root.join("whisper-cli");
    let model = root.join("model.bin");
    fs::write(
        &executable,
        "#!/bin/sh\nout=\nfile=\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = --output-file ]; then out=$2; shift 2; elif [ \"$1\" = --file ]; then file=$2; shift 2; else shift; fi\ndone\n[ \"$(wc -c < \"$file\")\" -eq 60 ] || exit 4\nprintf 'Rosehip House, clip status\\n' > \"${out}.txt\"\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&model, b"bounded whisper clip model fixture").unwrap();
    let adapter = WhisperDiscovery::inspect(&executable, &model)
        .unwrap()
        .initialize(WhisperLimits {
            maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
            maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
            threads: 2,
            timeout: Duration::from_secs(2),
        })
        .unwrap();
    let mut host = StdHost::new_with_whisper_clip_speech_recognition(
        StdHostConfig {
            host_id: HostId::from("whisper-clip-plan-play-host"),
            boot_id: BootId::from("whisper-clip-plan-play-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        adapter,
    )
    .unwrap();
    host.attach_proof_pcm_clip_source(
        crate::installed_std::test_local_model_io::recorded_house_audio_clip().unwrap(),
    )
    .unwrap();

    let mut catalog = crate::installed_std::test_catalog();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut catalog).unwrap();
    crate::installed_std::test_local_model_io::install_house_source_catalog(
        &mut startup,
        &mut catalog,
    );
    crate::installed_std::test_local_model_io::install_house_text_sink_catalog(
        &mut startup,
        &mut catalog,
    );
    let source = format!(
        "form whisper_clip_proof {{\n audio: {}\n recognize: speech/recognize-clip\n text: speech/recognition-to-text\n sink: {}\n audio.value >> recognize.clip\n recognize.result >> text.result\n text.text >> sink.value\n}}\n",
        crate::installed_std::test_local_model_io::HOUSE_AUDIO_CLIP_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_TEXT_SINK_KIND,
    );
    let form = conduit_form::parse(&source, &catalog).unwrap();
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &advertisements).unwrap();
    let plan = conduit_planner::plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::RECOGNITION_RESULT_QUEUE_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.implementation_id.as_str()
                == conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION
        })
        .unwrap();
    assert_eq!(placement.resources.len(), 1);
    let report = host
        .run_fragment_to(
            plan.fragments[0].clone(),
            &mut Vec::with_capacity(1_024),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert_eq!(report.speech_recognition.len(), 1);
    assert_eq!(
        report.speech_recognition[0].implementation_id.as_str(),
        conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION
    );
    assert_eq!(report.speech_recognition[0].text_bytes, 26);
    fs::remove_dir_all(root).unwrap();
}
