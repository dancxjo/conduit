#![cfg(feature = "semantic-bindings")]
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod occurrence_intent;
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{intent_fundamental::*, semantic::*};
fn anchor(origin: u64) -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(origin).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn control(hz: u64) -> AudioFrequencyControl {
    AudioFrequencyControl::new(
        anchor(1),
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "target fixture".into(),
            Some("1".into()),
        )
        .unwrap(),
        AudioFrequencyHz::new(1, hz).unwrap(),
    )
    .unwrap()
}
fn text() -> conduit_language::LanguageText {
    conduit_language::LanguageText::new(
        LanguageTextId::new("source".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        LanguageTextRevisionId::new("source revision".into()).unwrap(),
        "t".into(),
    )
    .unwrap()
}
fn intent(cycle: SpeechCycleSpecification) -> SpeechUtteranceIntent {
    let SpeechUtteranceIntentEvent::Segment(v) = occurrence_intent::segment(0) else {
        unreachable!()
    };
    let event = SpeechUtteranceIntentEvent::segment(
        v.occurrence().clone(),
        v.phone().clone(),
        v.phoneme().clone(),
        SpeechSegmentProsodyIntent::new(
            v.prosody().duration().clone(),
            cycle,
            v.prosody().relative_intensity().clone(),
        )
        .unwrap(),
        v.provenance().clone(),
        v.sources().clone(),
        v.stress().clone(),
        v.word_position().clone(),
    )
    .unwrap();
    SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter([event]).unwrap(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        v.provenance().clone(),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
#[test]
fn target_keeps_exact_source_and_reciprocal_at_three_independent_rates() {
    let intent = intent(SpeechCycleSpecification::known(120, 1).unwrap());
    let text = text();
    let frequency = control(120);
    let occurrence = occurrence_intent::token(0, occurrence_intent::BASIS);
    let prepared = prepare_intent_fundamental(&intent, 0, &occurrence, &text, &frequency).unwrap();
    assert!(core::ptr::eq(prepared.intent(), &intent));
    assert!(core::ptr::eq(prepared.text(), &text));
    assert!(core::ptr::eq(prepared.control(), &frequency));
    assert_eq!(prepared.reverse().result(), frequency.value());
    assert_eq!(prepared.reciprocal().result().numerator_seconds(), &1);
    assert_eq!(prepared.reciprocal().result().denominator(), &120);
    for rate in [8000, 16000, 48000] {
        let basis =
            AudioSampleRateBasis::new(anchor(1), AudioFrameQuantization::Floor, rate).unwrap();
        let projection = prepared.at_rate(Some(&basis)).unwrap();
        assert_eq!(*projection.result().raw().whole_frames(), rate / 120);
        assert_eq!(*projection.result().raw().remainder_numerator(), rate % 120);
        assert_eq!(projection.original().basis(), &basis);
        assert!(!projection.executions().is_empty());
    }
    assert!(matches!(
        prepared.at_rate(None),
        Err(IntentFundamentalRefusal::MissingRate { event: 0 })
    ));
    let foreign =
        AudioSampleRateBasis::new(anchor(2), AudioFrameQuantization::Floor, 8000).unwrap();
    assert!(matches!(
        prepared.at_rate(Some(&foreign)),
        Err(IntentFundamentalRefusal::ForeignAnchor { event: 0 })
    ));
    let unsupported =
        AudioSampleRateBasis::new(anchor(1), AudioFrameQuantization::Floor, 192001).unwrap();
    assert!(matches!(
        prepared.at_rate(Some(&unsupported)),
        Err(IntentFundamentalRefusal::Projection { event: 0, .. })
    ));
}
#[test]
fn unresolved_mismatch_stale_foreign_and_absent_refuse() {
    let text = text();
    let frequency = control(120);
    let occurrence = occurrence_intent::token(0, occurrence_intent::BASIS);
    for cycle in [
        SpeechCycleSpecification::unknown(),
        SpeechCycleSpecification::unspecified(),
        SpeechCycleSpecification::not_applicable(),
    ] {
        let intent = intent(cycle.clone());
        assert!(
            matches!(prepare_intent_fundamental(&intent,0,&occurrence,&text,&frequency),Err(IntentFundamentalRefusal::Unresolved{event:0,specification}) if specification==cycle)
        );
    }
    for cycle in [
        SpeechCycleSpecification::known(121, 1).unwrap(),
        SpeechCycleSpecification::known(240, 2).unwrap(),
    ] {
        let intent = intent(cycle);
        assert!(matches!(
            prepare_intent_fundamental(&intent, 0, &occurrence, &text, &frequency),
            Err(IntentFundamentalRefusal::CycleMismatch { event: 0 })
        ));
    }
    let intent = intent(SpeechCycleSpecification::known(120, 1).unwrap());
    let before = intent.clone().encode().unwrap();
    assert!(matches!(
        prepare_intent_fundamental(&intent, 1, &occurrence, &text, &frequency),
        Err(IntentFundamentalRefusal::MissingEvent { event: 1 })
    ));
    let foreign = occurrence_intent::token(1, occurrence_intent::BASIS);
    assert!(matches!(
        prepare_intent_fundamental(&intent, 0, &foreign, &text, &frequency),
        Err(IntentFundamentalRefusal::ForeignOccurrence { event: 0 })
    ));
    let stale = conduit_language::LanguageText::new(
        text.identity().clone(),
        text.language().clone(),
        LanguageTextRevisionId::new("stale".into()).unwrap(),
        text.text().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_intent_fundamental(&intent, 0, &occurrence, &stale, &frequency),
        Err(IntentFundamentalRefusal::Source {
            event: 0,
            source: 0,
            ..
        })
    ));
    assert_eq!(intent.encode().unwrap(), before);
    assert!(AudioFrequencyHz::new(1, 0).is_err());
    let amplitude = AudioRelativeAmplitude::new(1, 120).unwrap();
    assert!(AudioFrequencyHz::decode(&amplitude.encode().unwrap()).is_err());
    assert!(AudioFrequencyHz::new(0, 120).is_err());
    assert!(AudioSampleRateBasis::new(anchor(1), AudioFrameQuantization::Floor, 0).is_err());
}
#[test]
fn supplied_observation_keeps_different_measured_value_separate_from_target() {
    let intent = intent(SpeechCycleSpecification::known(120, 1).unwrap());
    let original = intent.clone().encode().unwrap();
    let text = text();
    let frequency = control(120);
    let occurrence = occurrence_intent::token(0, occurrence_intent::BASIS);
    let target = prepare_intent_fundamental(&intent, 0, &occurrence, &text, &frequency).unwrap();
    let measured = AudioFrequencyControl::new(
        anchor(1),
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Measured,
            "supplied fixture observation".into(),
            None,
        )
        .unwrap(),
        AudioFrequencyHz::new(1, 110).unwrap(),
    )
    .unwrap();
    let at =
        AudioTrajectoryQuery::new(anchor(1), AudioExactTimeOffset::new(10, 1).unwrap()).unwrap();
    let span = SpeechExactDuration::new(100, 1).unwrap();
    let provenance = SpeechEvidenceProvenance::new(
        "supplied fixture observation".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap();
    let observation =
        bind_observed_intent_fundamental(&target, &measured, &at, &span, &provenance, None)
            .unwrap();
    assert_eq!(*observation.observed().value().numerator_hz(), 110);
    assert_eq!(*observation.target().control().value().numerator_hz(), 120);
    assert_eq!(*observation.reciprocal().result().denominator(), 110);
    assert!(observation.confidence().is_none());
    assert_eq!(
        observation.target().intent().clone().encode().unwrap(),
        original
    );
    let foreign =
        AudioTrajectoryQuery::new(anchor(2), AudioExactTimeOffset::new(10, 1).unwrap()).unwrap();
    assert!(matches!(
        bind_observed_intent_fundamental(&target, &measured, &foreign, &span, &provenance, None),
        Err(IntentFundamentalRefusal::ForeignAnchor { event: 0 })
    ));
}

#[allow(dead_code)]
#[path = "common/correspondence.rs"]
mod correspondence_fixture;
#[test]
fn authoritative_entry_keeps_original_inventory_variety_revision_and_source() {
    use conduit_speech::{
        correspondence::PreparedSpeechCorrespondence, ipa_inventory::PreparedIpaInventory,
    };
    use correspondence_fixture as f;
    let original = f::intent();
    let events = original.events().as_slice().iter().map(|event| {
        let SpeechUtteranceIntentEvent::Segment(v) = event else {
            unreachable!()
        };
        SpeechUtteranceIntentEvent::segment(
            v.occurrence().clone(),
            v.phone().clone(),
            v.phoneme().clone(),
            SpeechSegmentProsodyIntent::new(
                v.prosody().duration().clone(),
                SpeechCycleSpecification::known(120, 1).unwrap(),
                v.prosody().relative_intensity().clone(),
            )
            .unwrap(),
            v.provenance().clone(),
            v.sources().clone(),
            v.stress().clone(),
            v.word_position().clone(),
        )
        .unwrap()
    });
    let intent = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        original.inventory_id().clone(),
        original.language().clone(),
        original.provenance().clone(),
        original.revision_id().clone(),
        original.utterance_id().clone(),
    )
    .unwrap();
    let (inventory, profile, text, phonemes, phones) = (
        f::inventory(),
        f::profile(),
        f::text(),
        f::phonemes(),
        f::phones(),
    );
    let (phone, phoneme) = f::bindings();
    let admitted_inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[phone],
        &[phoneme],
    )
    .unwrap();
    let alignment = f::alignment(f::groups());
    let carrier = PreparedSpeechCorrespondence::prepare(
        &intent,
        &phonemes,
        &phones,
        &text,
        &alignment,
        &admitted_inventory,
    )
    .unwrap();
    let frequency = control(120);
    let prepared = prepare_corresponding_intent_fundamental(&carrier, 0, &frequency).unwrap();
    assert!(core::ptr::eq(prepared.carrier(), &carrier));
    assert!(core::ptr::eq(
        prepared.fundamental().intent(),
        carrier.intent()
    ));
    assert!(core::ptr::eq(prepared.fundamental().text(), carrier.text()));
    assert_eq!(
        prepared.carrier().inventory().basis().variety(),
        profile.variety()
    );
    assert_eq!(
        prepared.carrier().inventory().basis().revision(),
        profile.revision()
    );
    assert_eq!(
        prepared.carrier().alignment().inventory_basis(),
        admitted_inventory.basis()
    );
    assert!(matches!(
        prepare_corresponding_intent_fundamental(&carrier, 99, &frequency),
        Err(IntentFundamentalRefusal::MissingEvent { event: 99 })
    ));
    let measured = control(110);
    let at =
        AudioTrajectoryQuery::new(anchor(1), AudioExactTimeOffset::new(1, 0).unwrap()).unwrap();
    let span = SpeechExactDuration::new(1, 1).unwrap();
    let provenance = f::provenance();
    let observation = prepared
        .observe(&measured, &at, &span, &provenance, None)
        .unwrap();
    assert!(core::ptr::eq(observation.carrier(), &carrier));
    assert_eq!(
        observation.carrier().inventory().basis().variety(),
        profile.variety()
    );
    assert_eq!(
        *observation.observation().observed().value().numerator_hz(),
        110
    );
    assert_eq!(
        *observation
            .observation()
            .target()
            .control()
            .value()
            .numerator_hz(),
        120
    );
    let foreign_at =
        AudioTrajectoryQuery::new(anchor(2), AudioExactTimeOffset::new(1, 0).unwrap()).unwrap();
    assert!(matches!(
        prepared.observe(&measured, &foreign_at, &span, &provenance, None),
        Err(IntentFundamentalRefusal::ForeignAnchor { event: 0 })
    ));
    let source_scope = SpeechPhoneAcousticScope::new(
        anchor(1),
        admitted_inventory.basis().clone(),
        f::reference("phones", 0),
        f::source(),
    )
    .unwrap();
    let target = SpeechFundamentalTarget::new(frequency.clone(), source_scope.clone()).unwrap();
    // Public authored transport retains the exact imported owner contracts.
    let mut catalog = conduit_plot::StartupCatalog::new();
    conduit_speech::authoring::install(&mut catalog).unwrap();
    let source = "plot fundamental/copy (\n >> value: SpeechFundamentalTarget\n result: SpeechFundamentalTarget >>\n) = (.)";
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &catalog)
            .unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "fundamental/copy",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    let conduit_core::ConfigurationValue::Text(encoded) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("authored expression")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    assert_eq!(
        program.evaluate(&target.clone().encode().unwrap()).unwrap(),
        target.clone().encode().unwrap()
    );
    let wrong_dimension = "plot fundamental/wrong (\n >> value: AudioCycleDuration\n result: AudioFrequencyHz >>\n) = (.)";
    let refused = match conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(wrong_dimension),
        &catalog,
    ) {
        Err(_) => true,
        Ok(checked) => conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "fundamental/wrong",
            &conduit_plot::ProfileCatalog::new(),
        )
        .is_err(),
    };
    assert!(
        refused,
        "cycle is not a frequency without the explicit generic Audio conversion"
    );
    let target_only = SpeechFundamentalBinding::new(None, target.clone()).unwrap();
    let prepared_target = prepare_phone_fundamental(&carrier, 0, &target_only).unwrap();
    assert!(prepared_target.observation().is_none());
    assert!(prepared_target.observed_cycle().is_none());
    assert_eq!(
        prepared_target.original().inventory().basis(),
        admitted_inventory.basis()
    );
    let observed = SpeechFundamentalObservation::new(
        at.clone(),
        Some(f::confidence()),
        measured.clone(),
        f::provenance(),
        source_scope.clone(),
        AudioExactTimeOffset::new(1, 1).unwrap(),
    )
    .unwrap();
    let binding = SpeechFundamentalBinding::new(Some(observed.clone()), target.clone()).unwrap();
    let original_bytes = intent.clone().encode().unwrap();
    let text_bytes = text.clone().encode().unwrap();
    let receipt = prepare_phone_fundamental(&carrier, 0, &binding).unwrap();
    assert!(core::ptr::eq(receipt.original(), &carrier));
    assert_eq!(receipt.observation().unwrap(), &observed);
    assert_eq!(
        receipt.binding().target().control().value(),
        frequency.value()
    );
    assert_eq!(receipt.observed_cycle().unwrap().input(), measured.value());
    assert_eq!(
        receipt.observed_cycle().unwrap().result(),
        &AudioCycleDuration::new(110, 1).unwrap()
    );
    assert!(!receipt
        .observed_cycle()
        .unwrap()
        .source_program_hex()
        .is_empty());
    assert_eq!(intent.clone().encode().unwrap(), original_bytes);
    assert_eq!(text.clone().encode().unwrap(), text_bytes);
    let foreign_scope = SpeechPhoneAcousticScope::new(
        anchor(1),
        admitted_inventory.basis().clone(),
        f::reference("phones", 1),
        f::source(),
    )
    .unwrap();
    let foreign_target = SpeechFundamentalTarget::new(frequency.clone(), foreign_scope).unwrap();
    let foreign_binding = SpeechFundamentalBinding::new(None, foreign_target.clone()).unwrap();
    assert!(matches!(
        prepare_phone_fundamental(&carrier, 0, &foreign_binding),
        Err(IntentFundamentalRefusal::ForeignOccurrence { event: 0 })
    ));
    assert!(SpeechFundamentalBinding::new(Some(observed.clone()), foreign_target).is_err());
    assert!(SpeechFundamentalTarget::new(
        AudioFrequencyControl::new(
            anchor(2),
            frequency.provenance().clone(),
            *frequency.value()
        )
        .unwrap(),
        source_scope.clone()
    )
    .is_err());
    assert!(SpeechFundamentalObservation::new(
        foreign_at.clone(),
        None,
        measured.clone(),
        f::provenance(),
        source_scope.clone(),
        AudioExactTimeOffset::new(1, 1).unwrap(),
    )
    .is_err());
    let other_source = LanguageTextSegmentRef::new(
        *f::source().kind(),
        f::source().language().clone(),
        f::source().range().clone(),
        f::source().revision_id().clone(),
        LanguageTextId::new("foreign-text".into()).unwrap(),
    )
    .unwrap();
    let source_mismatch = SpeechPhoneAcousticScope::new(
        anchor(1),
        admitted_inventory.basis().clone(),
        f::reference("phones", 0),
        other_source,
    )
    .unwrap();
    let source_binding = SpeechFundamentalBinding::new(
        None,
        SpeechFundamentalTarget::new(frequency.clone(), source_mismatch).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepare_phone_fundamental(&carrier, 0, &source_binding),
        Err(IntentFundamentalRefusal::CarrierSource { event: 0 })
    ));
    let stale = conduit_language::LanguageText::new(
        text.identity().clone(),
        text.language().clone(),
        LanguageTextRevisionId::new("stale".into()).unwrap(),
        text.text().clone(),
    )
    .unwrap();
    assert!(PreparedSpeechCorrespondence::prepare(
        &intent,
        &phonemes,
        &phones,
        &stale,
        &alignment,
        &admitted_inventory
    )
    .is_err());
}
