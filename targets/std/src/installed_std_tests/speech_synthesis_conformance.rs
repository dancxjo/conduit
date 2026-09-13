use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    BaseImplementationId, BootId, HostId, ObservationKind, OfferGeneration, TerminalDisposition,
};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn unchanged_speech_form_streams_several_blocks_through_ordinary_plan_and_play() {
    let mut catalog = installed_std::test_catalog();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut catalog)
        .expect("speech synthesis catalog is exact");
    let form = conduit_form::parse(
        "form bounded_speech {\n synthesize: speech/synthesize(maximum-output-bytes = 32768)\n sink: conduit-proof/speech-pcm-sink\n \"Rosehip House is ready.\" > synthesize.text\n synthesize.audio > sink.audio\n}\n",
        &catalog,
    )
    .expect("provider-neutral speech Form parses");
    let mut host = host("deterministic-speech-host");
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &advertisements)
        .expect("initialized deterministic speech offer is selected");
    let plan = conduit_planner::plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::MAXIMUM_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("provider-neutral speech Form plans through the std offer");
    let synthesis = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_KIND)
        .expect("speech placement exists");
    assert_eq!(
        synthesis.execution_profile_id.as_str(),
        conduit_std_offers::DETERMINISTIC_SPEECH_PROFILE
    );
    assert_eq!(synthesis.configuration[0].key, "maximum-output-bytes");

    let report = host
        .run_fragment_to(
            plan.fragments[0].clone(),
            &mut Vec::with_capacity(1_024),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect("three fake provider blocks stream through the production kernel");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn initialized_piper_runs_the_unchanged_form_through_ordinary_plan_and_play() {
    let root = std::env::temp_dir().join(format!("conduit-piper-plan-play-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let executable = root.join("piper-fixture");
    let model = root.join("voice.onnx");
    let config = root.join("voice.onnx.json");
    fs::write(
        &executable,
        "#!/bin/sh\ncat >/dev/null\ndd if=/dev/zero bs=1 count=678 2>/dev/null\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&model, b"bounded model fixture").unwrap();
    fs::write(&config, br#"{"audio":{"sample_rate":22050}}"#).unwrap();
    let adapter = crate::hosted_speech::PiperDiscovery::inspect(&executable, &model, &config, None)
        .unwrap()
        .initialize(crate::hosted_speech::PiperLimits {
            maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
            maximum_frames: conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2),
            maximum_blocks: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
            timeout: std::time::Duration::from_secs(2),
        })
        .unwrap();
    let expected_model_sha256 = adapter.discovery().model_sha256.clone();
    let mut host = crate::StdHost::new_with_piper_speech(
        crate::StdHostConfig {
            host_id: HostId::from("real-piper-host"),
            boot_id: BootId::from("real-piper-boot"),
            offer_generation: OfferGeneration(1),
        },
        crate::StdHostComposition::reference(),
        adapter,
    )
    .unwrap();
    assert!(host.advertisement().resources.iter().any(|resource| {
        resource.class_id.as_str() == conduit_std_offers::PIPER_PROCESS_RESOURCE_CLASS
            && resource.capacity_units == 1
    }));

    let mut catalog = installed_std::test_catalog();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut catalog).unwrap();
    let form = conduit_form::parse(
        "form real_speech {\n synthesize: speech/synthesize(maximum-output-bytes = 32768)\n sink: conduit-proof/speech-pcm-sink\n \"Rosehip House\" > synthesize.text\n synthesize.audio > sink.audio\n}\n",
        &catalog,
    )
    .unwrap();
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
            connection_byte_capacity: conduit_std_offers::PIPER_PCM_BLOCK_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let synthesis = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_KIND)
        .unwrap();
    assert_eq!(
        synthesis.implementation_id.as_str(),
        conduit_std_offers::PIPER_SPEECH_IMPLEMENTATION
    );
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
    assert_eq!(report.speech_synthesis.len(), 1);
    let speech = &report.speech_synthesis[0];
    assert_eq!(speech.plan_id, plan.fragments[0].plan_id);
    assert_eq!(
        speech.implementation_id.as_str(),
        conduit_std_offers::PIPER_SPEECH_IMPLEMENTATION
    );
    assert_eq!(speech.model_sha256, expected_model_sha256);
    assert_eq!((speech.frames, speech.blocks), (339, 3));
    assert_eq!(speech.text_sha256.len(), 64);
    assert_eq!(speech.pcm_sha256.len(), 64);
    let kernel = report.kernel.as_ref().unwrap();
    assert_eq!(speech.active_play_id, kernel.active_play_id);
    // Process creation is an admitted hosted effect and remains visible in the
    // execution report rather than being misreported as allocation-free Play.
    assert!(kernel.post_play_start_allocations > 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    let _ = fs::remove_dir_all(root);
}
