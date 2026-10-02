//! Deterministic long-stream fixture through ordinary planning and Host Calls.
use crate::{StdHost, StdHostComposition, StdHostConfig, TimerAdapter};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}

#[test]
fn sparse_speech_pipeline_retains_host_inputs_for_entire_long_stream() {
    let files =
        Fixture(std::env::temp_dir().join(format!("conduit-wav-budget-{}", std::process::id())));
    fs::create_dir(&files.0).unwrap();
    let data = files.0.join("espeak-ng-data");
    fs::create_dir(&data).unwrap();
    fs::write(data.join("voice"), b"deterministic test voice").unwrap();
    let engine = files.0.join("libespeak-ng.so.1.0");
    fs::write(&engine, b"deterministic test engine identity").unwrap();
    std::os::unix::fs::symlink("libespeak-ng.so.1.0", files.0.join("libespeak-ng.so.1")).unwrap();
    // 200 full 25-frame blocks: enough to fill and repeatedly drain every
    // pipeline stage. The executable fixture is not a real speech proof.
    let frames = 5_000u32;
    let pcm_bytes = frames * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&22_050u32.to_le_bytes());
    wav.extend_from_slice(&44_100u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&pcm_bytes.to_le_bytes());
    for _ in 0..frames {
        wav.extend_from_slice(&1234i16.to_le_bytes());
    }
    let escaped: String = wav.iter().map(|byte| format!("\\{byte:03o}")).collect();
    let executable = files.0.join("fixture-espeak");
    fs::write(&executable, format!("#!/bin/sh\nprintf '{escaped}'\n")).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let discovery = crate::hosted_speech_synthesis::EspeakDiscovery::inspect(
        &executable,
        &data,
        "en-us",
        &[engine],
    )
    .unwrap();
    let config = StdHostConfig {
        host_id: "host/wav-budget".into(),
        boot_id: "boot/wav-budget".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    };
    let adapter = discovery
        .initialize(
            config.host_id.clone(),
            config.boot_id.clone(),
            config.offer_generation,
            "grant/speech".into(),
            Duration::from_secs(5),
        )
        .unwrap();
    let destination = files.0.join("output.wav");
    let artifact = crate::hosted_wav_artifact::WavArtifactSelection::new(
        &destination,
        config.boot_id.clone(),
        config.offer_generation,
    )
    .unwrap();
    let mut host = StdHost::new_with_composition(config, StdHostComposition::minimal().with_text());
    host.attach_espeak_speech_and_wav_artifact(adapter, artifact)
        .unwrap();
    let source = "plot bounded_speech {\n voice: speech/synthesize(maximum-output-bytes = 131072)\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\")\n artifact: audio/play\n \"Hello.\" >> voice.text\n voice.audio >> convert.audio\n convert.converted >> artifact.audio\n}.\n";
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profiles).unwrap();
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "bounded_speech", &profiles).unwrap();
    let hosts = [host.advertisement().clone()];
    let mut placements =
        conduit_planner::default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    // Test builds also advertise the short deterministic provider. Explicitly
    // select this exact fixture-backed adapter through the ordinary planner.
    for (gear, placement) in &mut placements.by_gear {
        if gear.as_str().ends_with("/voice") {
            placement.capability_id = hosts[0]
                .capabilities
                .iter()
                .find(|offer| {
                    offer.implementation.implementation_id.as_str()
                        == conduit_std_offers::ESPEAK_SPEECH_IMPLEMENTATION
                })
                .unwrap()
                .capability_id
                .clone();
        }
    }
    let grants = [
        host.speech_synthesis_authority_grant().unwrap(),
        host.wav_artifact_authority_grant("grant/wav").unwrap(),
    ];
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|p| p.implementation_id.as_str() == conduit_std_offers::ESPEAK_SPEECH_IMPLEMENTATION));
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut Vec::new(), &mut NoTimer)
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|o| &o.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    let [artifact] = kernel.wav_artifacts.as_slice() else {
        panic!("exactly one retained WAV")
    };
    assert!(artifact.completed);
    assert_eq!(artifact.blocks, 200);
    assert_eq!(artifact.frames, (frames * 48_000).div_ceil(22_050));
    let retained = fs::read(destination).unwrap();
    assert_eq!(retained.len(), artifact.pcm_bytes as usize + 44);
    assert_eq!(&retained[..4], b"RIFF");
}
