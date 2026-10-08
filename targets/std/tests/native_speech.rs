//! Real installed native Back and exact external PCM Fore; no device provider.
use conduit_core::{
    BaseImplementationId, ConnectionTrack, ObservationKind, PortDirection, TerminalDisposition,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost,
    StdHostComposition, StdHostConfig, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
fn config() -> StdHostConfig {
    StdHostConfig {
        host_id: "native-speech-host".into(),
        boot_id: "native-speech-boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    }
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("native speech has no timer effect");
    }
}
#[derive(Default)]
struct Collector {
    pcm: Vec<u8>,
    next: u64,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        match value.track {
            ConnectionTrack::Payload => {
                assert_eq!(
                    value.value_kind,
                    conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID)
                );
                let (header, bytes) =
                    conduit_audio::PcmFrameHeader::decode_frame(&value.bytes).unwrap();
                assert_eq!(header.sample_rate_hz(), 8000);
                assert_eq!(header.start_frame(), self.next);
                assert_eq!(header.clock_id(), 7);
                self.next += u64::from(header.frame_count());
                self.pcm.extend_from_slice(bytes);
            }
            ConnectionTrack::NormalClose => {}
            _ => panic!("no abnormal/quiescent PCM effect promised"),
        }
        Ok(())
    }
}
const ENGLISH_REQUEST: &str = r#"{ language: "language/english", variety: some({ identity: "pronunciation/native-english@2", language: "language/english" }), variety_policy: exact_variety("") }"#;

fn source(request: &str) -> String {
    r#"plot native-voice (
 >> text: Text <= 512B
 audio: PcmFrames...| <= 285B >>
) {
 voice: speech/utterance(clock=7, language-request = REQUEST)
 text >> voice.text
 voice.audio >> audio
}.
"#
    .replace("REQUEST", request)
}

fn run(text: &str) -> (Collector, conduit_std_host::StdRunReport) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_speech::kernel::install(&mut startup, &mut profile).unwrap();
    let source = source(ENGLISH_REQUEST);
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "native-voice", &profile).unwrap();
    let mut host =
        StdHost::new_with_composition(config(), StdHostComposition::minimal().with_native_speech());
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: "text".into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 512,
            },
        ),
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: "audio".into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 285,
            },
        ),
    ]
    .into_iter()
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 512,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let mut collector = Collector::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "text".into(),
                track: ConnectionTrack::Payload,
                bytes: text.as_bytes().to_vec(),
            }],
            &mut collector,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    (collector, report)
}
#[test]
fn installed_host_runs_native_voice_and_closes_after_exact_pcm_delivery() {
    for text in ["This is a native speech synthesizer.", "007"] {
        let (collected, report) = run(text);
        let mut events =
            [conduit_speech::VoiceEvent::boundary(conduit_speech::VoiceBoundary::phrase);
                conduit_speech::MAXIMUM_EVENTS];
        let prepared = conduit_speech::pronounce(text, &mut events).unwrap();
        let mut renderer = conduit_speech::Renderer::prepare(prepared.events()).unwrap();
        let mut expected = Vec::new();
        while !renderer.is_complete() {
            let mut block = [0_i16; conduit_speech::MAXIMUM_BLOCK_FRAMES];
            let count = renderer.render(&mut block).unwrap();
            for sample in &block[..count] {
                expected.extend_from_slice(&sample.to_le_bytes());
            }
        }
        assert_eq!(collected.pcm, expected);

        let kernel = report.kernel.as_ref().unwrap();
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::BackCompleted));
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::RemoteOutputClosed));
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(report.observations.iter().any(|o| matches!(
            o.kind,
            ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )));
    }
}
#[test]
fn minimal_and_reference_compositions_truthfully_select_the_native_family() {
    let offers = |composition| {
        StdHost::new_with_composition(config(), composition)
            .advertisement()
            .capabilities
            .clone()
    };
    let expected = [
        conduit_speech::kernel::offer(),
        conduit_speech::ipa_contract::ipa_offer(false),
        conduit_speech::ipa_contract::ipa_offer(true),
    ];
    assert!(!offers(StdHostComposition::minimal())
        .iter()
        .any(|offer| expected
            .iter()
            .any(|native| offer.kind_id == native.kind_id)));
    let selected = offers(StdHostComposition::minimal().with_native_speech());
    let baseline = offers(StdHostComposition::minimal());
    let added: Vec<_> = selected
        .iter()
        .filter(|offer| !baseline.contains(offer))
        .collect();
    assert_eq!(added, expected.iter().collect::<Vec<_>>());
    let reference = offers(StdHostComposition::reference());
    assert!(expected.iter().all(|offer| reference.contains(offer)));
}

#[test]
fn installed_native_voice_refuses_other_languages_and_pronunciation_varieties_before_play() {
    use conduit_language::LanguageCoverageRefusal;
    use conduit_planner::PlannerError;
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_speech::kernel::install(&mut startup, &mut profile).unwrap();
    let host =
        StdHost::new_with_composition(config(), StdHostComposition::minimal().with_native_speech());
    let hosts = [host.advertisement().clone()];
    for (request, language, expected) in [
        (
            r#"{ language: "language/french", variety: none(""), variety_policy: language_sufficient("") }"#,
            "language/french",
            LanguageCoverageRefusal::Language,
        ),
        (
            r#"{ language: "language/english", variety: some({ identity: "pronunciation/other-english", language: "language/english" }), variety_policy: exact_variety("") }"#,
            "language/english",
            LanguageCoverageRefusal::Variety,
        ),
        (
            r#"{ language: "language/english", variety: none(""), variety_policy: language_sufficient("") }"#,
            "language/english",
            LanguageCoverageRefusal::MissingVariety,
        ),
    ] {
        let checked =
            check_syntax_document(&parse_syntax_document(&source(request)), &startup).unwrap();
        let authored =
            expand_canonical_plot_for_authoring(&checked, "native-voice", &profile).unwrap();
        let error =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap_err();
        let PlannerError::LanguageCoverageUnsatisfied(evidence) = error else {
            panic!("expected exact coverage refusal: {error:?}");
        };
        assert_eq!(evidence.requirements[0].request.language().get(), language);
        assert!(evidence.candidates.iter().any(|candidate| candidate
            .checks
            .iter()
            .any(|check| check.result == Err(expected))));
    }
}
