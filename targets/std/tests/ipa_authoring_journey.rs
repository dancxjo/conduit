use conduit_core::{BaseImplementationId, ConnectionTrack, PortDirection};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_speech::semantic::*;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig,
    TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("IPA constructor has no timer");
    }
}
#[derive(Default)]
struct Replies(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Replies {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
fn try_invoke_raw(source: &str) -> Result<(Vec<u8>, conduit_core::Plan), String> {
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "ipa-host".into(),
            boot_id: "ipa-boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal().with_native_speech(),
    );
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_std_host::install_ipa_catalog(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup)
        .map_err(|error| format!("{error:?}"))?;
    let authored = expand_canonical_plot_for_authoring(&checked, "ipa-authoring", &profiles)
        .map_err(|error| format!("{error:?}"))?;
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [(
        conduit_planner::ForeBoundaryKey {
            direction: PortDirection::Output,
            front_port_id: "outcome".into(),
            track: ConnectionTrack::Payload,
        },
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 262144,
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
            connection_byte_capacity: 262144,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let mut replies = Replies::default();
    let report = host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[],
        &mut replies,
        &mut Vec::new(),
        &mut NoTimer,
    )?;
    let kernel = report.kernel.expect("ordinary installed kernel");
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested)
            .count(),
        1
    );
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted)
            .count(),
        1
    );
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(replies.0.len(), 1);
    Ok((replies.0.remove(0), plan))
}
fn invoke_raw(source: &str) -> (Vec<u8>, conduit_core::Plan) {
    try_invoke_raw(source).unwrap()
}
#[test]
fn quoted_unicode_constructor_runs_actual_standard_hostcall_without_inventory() {
    let source = include_str!("../../../semantics/speech/examples/phonetic_ipa.conduit");
    let (bytes, _) = invoke_raw(source);
    let outcome = SpeechPhoneticIpaOutcome::decode(&bytes).unwrap();
    let SpeechPhoneticIpaOutcome::Admitted(value) = outcome else {
        panic!("Unicode phonetic constructor admitted");
    };
    assert_eq!(value.original(), "ˈt͡ʃtʰn̩ãː.ˌd͡ʒ");
    assert_eq!(value.units().len(), 9);
    assert_eq!((*value.units()[1].start(), *value.units()[1].end()), (2, 7));
    let first = &value.original()[2..7];
    assert_eq!(first, "t͡ʃ");
    assert_eq!(first.chars().count(), 3);
    let transcription = SpeechPhoneticIpaTranscription::new(
        value.original().clone(),
        value.profile().clone(),
        value.provenance().clone(),
        value.units().clone(),
    )
    .unwrap();
    let span = conduit_speech::ipa_phonetic::phonetic_unit_source_span(&transcription, 1).unwrap();
    assert_eq!(
        (
            span.byte_start,
            span.byte_end,
            span.scalar_start,
            span.scalar_end
        ),
        (2, 7, 1, 4)
    );
    let restored =
        SpeechPhoneticIpaTranscription::decode(&transcription.encode().unwrap()).unwrap();
    assert_eq!(
        conduit_speech::ipa_phonetic::phonetic_unit_source_span(&restored, 1).unwrap(),
        span
    );
    assert_eq!(value.units()[4].source_spelling().get(), "ã");
    assert_eq!(value.profile().units()[4].spelling().get(), "ã");
}
#[test]
fn orphan_mark_and_invalid_boundaries_return_located_native_outcomes() {
    let source = include_str!("../../../semantics/speech/examples/phonetic_ipa.conduit");
    for (text, start, end) in [("t͡ʃ̃", 5, 7), ("ːt͡ʃ", 0, 2), ("t͡ʃ.", 5, 6)] {
        let authored = source.replace("ˈt͡ʃtʰn̩ãː.ˌd͡ʒ", text);
        let (bytes, _) = invoke_raw(&authored);
        let outcome = SpeechPhoneticIpaOutcome::decode(&bytes).unwrap();
        let SpeechPhoneticIpaOutcome::Refused(reason) = outcome else {
            panic!("invalid combination must refuse");
        };
        assert_eq!(*reason.byte_start(), start);
        assert_eq!(*reason.byte_end(), end);
    }
}

#[test]
fn phonemic_source_requires_exact_inventory_revision_and_membership() {
    let source = include_str!("../../../semantics/speech/examples/phonemic_ipa.conduit");
    let (bytes, _) = invoke_raw(source);
    let SpeechPhonemicIpaOutcome::Admitted(value) =
        SpeechPhonemicIpaOutcome::decode(&bytes).unwrap()
    else {
        panic!("exact phonemic basis admitted");
    };
    assert_eq!(value.phonemes()[0].get(), "phoneme/a");
    assert_eq!(value.notation().transcription().original(), "t͡ʃ");
    assert!(SpeechPhoneticIpaOutcome::decode(&bytes).is_err());
    for (authored, expected) in [
        (
            source.replace(
                "identity: \"inventory/a\", language:",
                "identity: \"inventory/foreign\", language:",
            ),
            SpeechIpaRefusalReason::Inventory,
        ),
        (
            source.replace(
                "}, revision: \"revision/1\", variety:",
                "}, revision: \"revision/2\", variety:",
            ),
            SpeechIpaRefusalReason::Revision,
        ),
        (
            source.replace("original: \"t͡ʃ\"", "original: \"ã\""),
            SpeechIpaRefusalReason::Membership,
        ),
        (
            source.replace("ipa: \"t͡ʃ\"", "ipa: \"ã\""),
            SpeechIpaRefusalReason::Membership,
        ),
        (
            source.replace(
                "}, revision: \"revision/1\", variety: { identity: \"variety/a\"",
                "}, revision: \"revision/1\", variety: { identity: \"variety/foreign\"",
            ),
            SpeechIpaRefusalReason::Variety,
        ),
    ] {
        let (bytes, _) = invoke_raw(&authored);
        let SpeechPhonemicIpaOutcome::Refused(value) =
            SpeechPhonemicIpaOutcome::decode(&bytes).unwrap()
        else {
            panic!("foreign basis must refuse");
        };
        assert_eq!(*value.reason(), expected);
    }
}

#[test]
fn quoted_ipa_cst_highlighter_and_literal_serialization_preserve_utf8() {
    for source in [
        include_str!("../../../semantics/speech/examples/phonetic_ipa.conduit"),
        include_str!("../../../semantics/speech/examples/phonemic_ipa.conduit"),
    ] {
        let cst = parse_syntax_document(source);
        assert!(cst.diagnostics.is_empty(), "{:?}", cst.diagnostics);
        assert_eq!(cst.round_trip(), source);
        assert_eq!(
            cst.tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<String>(),
            source
        );
        let spans = conduit_plot::highlight_syntax(source).unwrap();
        assert_eq!(spans.first().unwrap().start, 0);
        assert_eq!(spans.last().unwrap().end, source.len());
        for pair in spans.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        for span in spans {
            assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
        }
    }
    let text = "t͡ʃd͡ʒtʰn̩ããˈˌː";
    let quoted = conduit_plot::text_startup_literal(text);
    assert_eq!(quoted, format!("\"{text}\""));
}

#[test]
fn constructor_unit_and_text_bounds_refuse_without_truncation() {
    let source = include_str!("../../../semantics/speech/examples/phonetic_ipa.conduit");
    let authored = source.replace("ˈt͡ʃtʰn̩ãː.ˌd͡ʒ", &"t͡ʃ".repeat(257));
    let (bytes, _) = invoke_raw(&authored);
    let SpeechPhoneticIpaOutcome::Refused(value) =
        SpeechPhoneticIpaOutcome::decode(&bytes).unwrap()
    else {
        panic!("unit bound refuses");
    };
    assert_eq!(*value.reason(), SpeechIpaRefusalReason::Capacity);
    let authored = source.replace("ˈt͡ʃtʰn̩ãː.ˌd͡ʒ", &"t".repeat(4097));
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_std_host::install_ipa_catalog(&mut startup, &mut profiles).unwrap();
    assert!(try_invoke_raw(&authored).is_err());
}

#[test]
fn single_phone_keeps_unknown_identity_and_cannot_decode_as_phoneme() {
    let source = include_str!("../../../semantics/speech/examples/phonetic_ipa.conduit")
        .replace("ˈt͡ʃtʰn̩ãː.ˌd͡ʒ", "t͡ʃ");
    let (_, plan) = invoke_raw(&source);
    let entry = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_speech::ipa_contract::PHONETIC_KIND)
        .unwrap()
        .configuration
        .iter()
        .find(|entry| entry.key == "request")
        .unwrap();
    let conduit_core::ConfigurationValue::Structured(value) = &entry.value else {
        panic!("typed IPA request");
    };
    let request = SpeechPhoneticIpaRequest::decode(value.canonical_value()).unwrap();
    let phone = conduit_speech::ipa_authoring::phone_from_ipa(&request).unwrap();
    assert!(matches!(phone.identity(), PhoneSpecification::Unknown));
    assert!(SpeechPhoneme::decode(&phone.encode().unwrap()).is_err());
}

#[test]
fn explicit_syllable_boundary_cannot_be_erased_inside_one_phoneme() {
    let source = include_str!("../../../semantics/speech/examples/phonemic_ipa.conduit")
        .replace("original: \"t͡ʃ\"", "original: \"t͡ʃã\"")
        .replace("notation: \"t͡ʃ\"", "notation: \"t͡ʃã\"")
        .replace("phoneme: \"phoneme/a\", units: [\"ipa/0\"]", "phoneme: \"phoneme/a\", units: [\"ipa/0\", \"ipa/1\"]")
        .replace("}], aliases: []", "}, { identity: \"ipa/boundary\", spelling: \".\", kind: syllable_boundary(\"\"), provenance: { source: manual(\"\"), method: \"explicit boundary\", version: none(\"\") } }], aliases: []");
    let (bytes, _) = invoke_raw(&source);
    assert!(matches!(
        SpeechPhonemicIpaOutcome::decode(&bytes).unwrap(),
        SpeechPhonemicIpaOutcome::Admitted(_)
    ));
    let crossing = source.replace("original: \"t͡ʃã\"", "original: \"t͡ʃ.ã\"");
    let (bytes, _) = invoke_raw(&crossing);
    let SpeechPhonemicIpaOutcome::Refused(value) =
        SpeechPhonemicIpaOutcome::decode(&bytes).unwrap()
    else {
        panic!("binding must not cross explicit syllable boundary");
    };
    assert_eq!(*value.reason(), SpeechIpaRefusalReason::Membership);
    assert_eq!(
        (
            *value.byte_start(),
            *value.byte_end(),
            *value.scalar_start(),
            *value.scalar_end()
        ),
        (5, 6, 3, 4)
    );
}
