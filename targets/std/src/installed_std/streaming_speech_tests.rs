//! Real engine proof is opt-in; ordinary tests validate the same authored graph.
use crate::{StdHost, StdHostComposition, StdHostConfig, TimerAdapter};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use std::{collections::BTreeMap, fs, time::Duration};
const TEXT: &str = "This Body keeps the clock you started. Change the interval, then inspect the connections to see how your action reaches the running work. You can pause the Body without erasing its history. When you return, inspect the current host and the new plan before starting again. If a presentation host disappears, the Body must show what stopped and which admitted route can continue. Your preference chooses among available Masks; it never invents a missing host.";
fn authoring() -> conduit_plot::ExpandedAuthoringPlot {
    let source = "plot narrated (\n >> text: Text...| <= 1024B\n) {\n commit: speech/commit-generated-text\n voice: speech/synthesize-stream(maximum-output-bytes=1323000, maximum-audio-millis=30000, maximum-segments=32)\n convert: audio/convert-pcm-profile(output-sample-rate-hz=48000, output-channel-layout=\"stereo-left-right\", maximum-blocks=32768, maximum-audio-millis=30000)\n artifact: audio/play(maximum-blocks=32768, maximum-audio-millis=30000)\n text >> commit.generated\n commit.segments >> voice.text\n voice.audio >> convert.audio\n convert.converted >> artifact.audio\n}.\n";
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profiles).unwrap();
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles).unwrap();
    conduit_tongues::install_speech_commit_catalog(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    expand_canonical_plot_for_authoring(&checked, "narrated", &profiles).unwrap()
}
#[test]
fn text_flow_enters_canonical_committed_segments_with_explicit_work_limits() {
    let plot = authoring();
    assert!(plot
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_contract_revision.as_str() == "conduit.speech/synthesize-stream@2"));
}
#[test]
fn external_value_is_not_an_implicit_closing_flow() {
    let source = "plot boundary (\n >> text: Text <= 1024B\n) {\n commit: speech/commit-generated-text\n text >> commit.generated\n}\n";
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_tongues::install_speech_commit_catalog(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, "boundary", &profiles).unwrap();
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "host/negative".into(),
            boot_id: "boot/negative".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    )
    .advertisement()
    .clone();
    host.capabilities
        .push(conduit_std_offers::generated_speech_commit_offer());
    let hosts = [host];
    let placements =
        conduit_planner::default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let result = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 2048,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &BTreeMap::from([(
            conduit_planner::ForeBoundaryKey {
                direction: conduit_core::PortDirection::Input,
                front_port_id: "text".into(),
                track: conduit_core::ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 1024,
            },
        )]),
    );
    assert!(
        matches!(result, Err(conduit_planner::PlannerError::InvalidPlotIdentity(message)) if message.contains("does not match its selected Back"))
    );
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}
#[test]
#[ignore = "requires installed eSpeak NG and data; produces a substantive real WAV, no playback/model inference"]
fn installed_espeak_narrates_a_chapter_through_incremental_plan_and_play() {
    let root =
        std::env::temp_dir().join(format!("conduit-streamed-chapter-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let config = StdHostConfig {
        host_id: "host/stream-proof".into(),
        boot_id: "boot/stream-proof".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    };
    let engine = fs::canonicalize("/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1").unwrap();
    let discovery = crate::hosted_speech_synthesis::EspeakDiscovery::inspect(
        std::path::Path::new("/usr/bin/espeak-ng"),
        std::path::Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
        "en-us",
        &[engine],
    )
    .unwrap();
    let adapter = discovery
        .initialize(
            config.host_id.clone(),
            config.boot_id.clone(),
            config.offer_generation,
            "grant/stream".into(),
            Duration::from_secs(30),
        )
        .unwrap();
    let artifact = crate::hosted_wav_artifact::WavArtifactSelection::new(
        root.join("chapter.wav"),
        config.boot_id.clone(),
        config.offer_generation,
    )
    .unwrap();
    let mut host = StdHost::new_with_composition(config, StdHostComposition::minimal().with_text());
    host.attach_espeak_speech_and_wav_artifact(adapter, artifact)
        .unwrap();
    let plot = authoring();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let grants = [
        host.streaming_speech_authority_grant().unwrap(),
        host.wav_artifact_authority_grant("grant/wav").unwrap(),
    ];
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 2048,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &BTreeMap::from([(
            conduit_planner::ForeBoundaryKey {
                direction: conduit_core::PortDirection::Input,
                front_port_id: "text".into(),
                track: conduit_core::ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 1024,
            },
        )]),
    )
    .unwrap();
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|p| p.implementation_id.as_str() == conduit_std_offers::ESPEAK_STREAM_IMPLEMENTATION));
    struct NoOutput;
    impl crate::ExternalForeOutputAdapter for NoOutput {
        fn deliver(&mut self, _: crate::ExternalForeDelivery) -> Result<(), String> {
            Err("unexpected external output".into())
        }
    }
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[crate::ExternalForeInput {
                front_port_id: "text".into(),
                track: conduit_core::ConnectionTrack::Payload,
                bytes: TEXT.as_bytes().to_vec(),
            }],
            &mut NoOutput,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|o| &o.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    let [artifact] = kernel.wav_artifacts.as_slice() else {
        panic!("one actual chapter artifact")
    };
    assert!(artifact.completed);
    assert!(
        (20 * 48000..=30 * 48000).contains(&artifact.frames),
        "actual duration={}s",
        f64::from(artifact.frames) / 48000.0
    );
    assert!(artifact.blocks > 3072);
    let wav = fs::read(root.join("chapter.wav")).unwrap();
    assert_eq!(wav.len(), artifact.pcm_bytes as usize + 44);
    assert!(wav[44..].iter().any(|byte| *byte != 0));
    fs::write(
        root.join("plan.json"),
        serde_json::to_vec_pretty(&plan).unwrap(),
    )
    .unwrap();
    fs::write(root.join("words.txt"), TEXT).unwrap();
    eprintln!("Actual streamed chapter retained at {}", root.display());
}
