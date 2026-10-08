//! Actual quoted Source → plan → installed preparation → fixed kernel Value.
use conduit_core::{
    BaseImplementationId, ConnectionTrack, PortDirection, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, ProfileCatalog, StartupCatalog,
};
use conduit_speech::{ipa_constructors::*, semantic::*};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig,
    TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
const PHONETIC: &str =
    include_str!("../../../semantics/speech/examples/ipa/quoted-transcriptions.conduit");
const PHONEMIC: &str =
    include_str!("../../../semantics/speech/examples/ipa/quoted-phonemic.conduit");
#[path = "ipa_constructors/readmission.rs"]
mod readmission;
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("IPA constructors have no timer effect");
    }
}
#[derive(Default)]
struct Collector {
    values: Vec<Vec<u8>>,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        match value.track {
            ConnectionTrack::Payload => self.values.push(value.bytes),
            ConnectionTrack::NormalClose => {}
            _ => panic!("no abnormal/quiescent effect"),
        }
        Ok(())
    }
}
#[test]
fn installed_constructors_emit_exact_typed_values_once_without_play_storage_growth() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_speech::authoring::install(&mut startup).unwrap();
    install(&mut startup, &mut profile).unwrap();
    let phone = PHONETIC
        .replace("speech/phonetic-from-ipa", "speech/phone-from-ipa")
        .replace("SpeechPhoneticTranscription", "SpeechPhoneNotation")
        .replace("ˈt͡ʃãː.n̩", "tʰ");
    let phoneme = PHONEMIC
        .replace("speech/phonemic-from-ipa", "speech/phoneme-from-ipa")
        .replace("SpeechPhonemicTranscription", "SpeechPhonemeNotation")
        .replace("ˈt͡ʃaː", "t͡ʃ");
    for (source, constructor) in [
        (phone.as_str(), IpaConstructor::Phone),
        (PHONETIC, IpaConstructor::Phonetic),
        (phoneme.as_str(), IpaConstructor::Phoneme),
        (PHONEMIC, IpaConstructor::Phonemic),
    ] {
        let syntax = parse_syntax_document(source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        validate_source(&syntax, &checked).unwrap();
        let authored =
            expand_canonical_plot_for_authoring(&checked, &syntax.plots[0].name.text, &profile)
                .unwrap();
        let expected =
            prepare_configuration(constructor, &authored.expanded.gears[0].configuration).unwrap();
        let mut host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: "ipa-host".into(),
                boot_id: "ipa-boot".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            StdHostComposition::minimal().with_native_speech(),
        );
        let hosts = [host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let boundaries = [(
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            },
        )]
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
                connection_byte_capacity: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
        assert!(plan
            .fragments
            .iter()
            .all(
                |fragment| fragment.placements.iter().all(|placement| placement
                    .host_calls
                    .is_empty()
                    && placement.resources.is_empty()
                    && placement.authority.is_empty())
            ));
        let mut collected = Collector::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[],
                &mut collected,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(collected.values, [expected.bytes().to_vec()]);
        let bytes = &collected.values[0];
        readmission::check(
            constructor,
            &authored.expanded.gears[0].configuration,
            bytes,
        );
        match constructor {
            IpaConstructor::Phone => assert_eq!(
                SpeechPhoneNotation::decode(bytes).unwrap().spelling().get(),
                "tʰ"
            ),
            IpaConstructor::Phonetic => assert_eq!(
                SpeechPhoneticTranscription::decode(bytes)
                    .unwrap()
                    .original(),
                "ˈt͡ʃãː.n̩"
            ),
            IpaConstructor::Phoneme => assert_eq!(
                SpeechPhonemeNotation::decode(bytes)
                    .unwrap()
                    .definition()
                    .identity()
                    .get(),
                "phoneme/ch"
            ),
            IpaConstructor::Phonemic => assert_eq!(
                SpeechPhonemicTranscription::decode(bytes)
                    .unwrap()
                    .original(),
                "ˈt͡ʃaː"
            ),
        }
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|sign| sign.kind == conduit_kernel::KernelEventKind::BackCompleted));
        assert!(kernel.playback.is_empty());
        assert!(kernel.kernel_sign.iter().all(|sign| !matches!(
            sign.kind,
            conduit_kernel::KernelEventKind::HostCallRequested
                | conduit_kernel::KernelEventKind::HostCallCancellationRequested
                | conduit_kernel::KernelEventKind::HostCallCompleted
                | conduit_kernel::KernelEventKind::BackFailed
        )));
    }
}
