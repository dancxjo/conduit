#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{declared_realization::*, semantic::*};
#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod fixture;
fn pid(s: &str) -> PhonemeId {
    PhonemeId::new(s.into()).unwrap()
}
fn evidence() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "declaration fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
fn intent(phoneme: PhonemeSpecification) -> SpeechUtteranceIntent {
    let sources = BoundedSequence::try_from_iter([LanguageSegmentRef::text(
        LanguageTextSegmentKind::Word,
        LanguageId::new("es".into()).unwrap(),
        LanguageTextRange::new(1, 0).unwrap(),
        LanguageTextRevisionId::new("source revision".into()).unwrap(),
        LanguageTextId::new("source".into()).unwrap(),
    )
    .unwrap()])
    .unwrap();
    let boundary = SpeechUtteranceIntentEvent::boundary(
        SpeechDurationSpecification::unknown(),
        SpeechBoundarySpecification::unknown(),
        evidence(),
        sources.clone(),
    )
    .unwrap();
    let segment = SpeechUtteranceIntentEvent::segment(
        LanguageSpeechTokenRef::new(
            SpeechInventoryId::new("inventory".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
            91,
            SpeechSegmentRevisionId::new("revision".into()).unwrap(),
            SpeechSegmentSequenceId::new("sequence".into()).unwrap(),
            SpeechUtteranceId::new("utterance".into()).unwrap(),
        )
        .unwrap(),
        PhoneSpecification::known(fixture::id("phone/t")).unwrap(),
        phoneme,
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::unknown(),
            SpeechCycleSpecification::unknown(),
            SpeechIntensitySpecification::unknown(),
        )
        .unwrap(),
        evidence(),
        sources,
        StressSpecification::unknown(),
        SpeechPositionSpecification::unknown(),
    )
    .unwrap();
    SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter([boundary, segment]).unwrap(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        evidence(),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn allophone(phone: &str) -> SpeechPhonemeAllophone {
    SpeechPhonemeAllophone::new(
        BoundedSequence::try_from_iter([SpeechRuleCondition::NotCarefulStyle]).unwrap(),
        SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unknown(),
            StressSpecification::unknown(),
            SpeechSyllablePositionSpecification::unknown(),
            SpeechPositionSpecification::unknown(),
        )
        .unwrap(),
        fixture::id(phone),
        Some("rule".into()),
        SpeechRuleStatus::Optional,
    )
    .unwrap()
}
fn phoneme(
    identity: &str,
    default: Option<&str>,
    possible: Vec<&str>,
    allophones: Vec<SpeechPhonemeAllophone>,
) -> SpeechPhoneme {
    SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::try_from_iter(allophones).unwrap(),
        default.map(fixture::id),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        pid(identity),
        "t".into(),
        BoundedSequence::try_from_iter(possible.into_iter().map(fixture::id)).unwrap(),
        SpeechSegmentStatus::Core,
    )
    .unwrap()
}
fn inventory(phonemes: Vec<SpeechPhoneme>) -> SpeechInventory {
    SpeechInventory::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter(phonemes).unwrap(),
        BoundedSequence::try_from_iter([fixture::definition("phone/t")]).unwrap(),
    )
    .unwrap()
}
#[test]
fn retains_all_declaration_paths_without_selecting_or_discharging_context() {
    let intent = intent(PhonemeSpecification::known(pid("phoneme/t")).unwrap());
    let inventory = inventory(vec![phoneme(
        "phoneme/t",
        Some("phone/t"),
        vec!["other", "phone/t"],
        vec![allophone("phone/t"), allophone("phone/t")],
    )]);
    let result = resolve_declared_intent_realization(&intent, 1, &inventory).unwrap();
    assert!(core::ptr::eq(result.source().intent(), &intent));
    assert!(core::ptr::eq(
        result.phoneme(),
        &inventory.phonemes().as_slice()[0]
    ));
    assert_eq!(result.source().event(), 1);
    assert_eq!(result.declarations().len(), 4);
    assert_eq!(result.checked_phoneme().requested(), &pid("phoneme/t"));
    assert!(matches!(
        result.declarations()[0].declaration(),
        PhoneDeclaration::Default(_)
    ));
    assert!(matches!(
        result.declarations()[1].declaration(),
        PhoneDeclaration::Possible { index: 1, .. }
    ));
    let PhoneDeclaration::Allophone {
        index: 0,
        declaration,
    } = result.declarations()[2].declaration()
    else {
        panic!()
    };
    assert!(core::ptr::eq(
        *declaration,
        &result.phoneme().allophones().as_slice()[0]
    ));
    assert_eq!(
        declaration.conditions().as_slice(),
        &[SpeechRuleCondition::NotCarefulStyle]
    );
    assert_eq!(declaration.status(), &SpeechRuleStatus::Optional);
    assert_eq!(declaration.confidence().get().value(), 0.25);
    for receipt in result.declarations() {
        assert_eq!(
            receipt.checked_phone().definition(),
            result.source().definition().identity()
        );
    }
}
#[test]
fn unresolved_phonemes_preserve_all_original_states_and_event() {
    let inventory = inventory(vec![phoneme("phoneme/t", Some("phone/t"), vec![], vec![])]);
    for state in [
        PhonemeSpecification::unknown(),
        PhonemeSpecification::unspecified(),
        PhonemeSpecification::not_applicable(),
        PhonemeSpecification::variable(BoundedSequence::try_from_iter([pid("phoneme/t")]).unwrap())
            .unwrap(),
        PhonemeSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            pid("phoneme/t"),
        )
        .unwrap(),
    ] {
        let intent = intent(state.clone());
        match resolve_declared_intent_realization(&intent, 1, &inventory) {
            Err(DeclaredRealizationRefusal {
                event: 1,
                reason: DeclaredRealizationReason::UnresolvedPhoneme(original),
            }) => assert_eq!(original, state),
            _ => panic!("must preserve state"),
        }
    }
}
#[test]
fn missing_duplicate_undeclared_and_wrong_events_remain_distinct() {
    let intent = intent(PhonemeSpecification::known(pid("phoneme/t")).unwrap());
    let cases = [
        inventory(vec![]),
        inventory(vec![
            phoneme("phoneme/t", None, vec![], vec![]),
            phoneme("phoneme/t", None, vec![], vec![]),
        ]),
        inventory(vec![phoneme(
            "phoneme/t",
            Some("other"),
            vec!["other"],
            vec![allophone("other")],
        )]),
    ];
    assert!(matches!(
        resolve_declared_intent_realization(&intent, 1, &cases[0]),
        Err(DeclaredRealizationRefusal {
            reason: DeclaredRealizationReason::MissingPhonemeDefinition,
            ..
        })
    ));
    assert!(matches!(
        resolve_declared_intent_realization(&intent, 1, &cases[1]),
        Err(DeclaredRealizationRefusal {
            reason: DeclaredRealizationReason::AmbiguousPhonemeDefinition,
            ..
        })
    ));
    assert!(matches!(
        resolve_declared_intent_realization(&intent, 1, &cases[2]),
        Err(DeclaredRealizationRefusal {
            reason: DeclaredRealizationReason::UndeclaredPhone,
            ..
        })
    ));
    assert!(matches!(
        resolve_declared_intent_realization(&intent, 0, &cases[0]),
        Err(DeclaredRealizationRefusal {
            event: 0,
            reason: DeclaredRealizationReason::Phone(
                conduit_speech::intent_inventory::IntentInventoryRefusal::BoundaryEvent
            )
        })
    ));
    assert!(matches!(
        resolve_declared_intent_realization(&intent, 2, &cases[0]),
        Err(DeclaredRealizationRefusal {
            event: 2,
            reason: DeclaredRealizationReason::Phone(
                conduit_speech::intent_inventory::IntentInventoryRefusal::MissingEvent
            )
        })
    ));
}

#[test]
fn all_seventeen_declarations_are_retained_and_native_identity_cannot_be_forged() {
    let intent = intent(PhonemeSpecification::known(pid("phoneme/t")).unwrap());
    let inventory = inventory(vec![phoneme(
        "phoneme/t",
        Some("phone/t"),
        vec!["phone/t"; 8],
        vec![allophone("phone/t"); 8],
    )]);
    let result = resolve_declared_intent_realization(&intent, 1, &inventory).unwrap();
    assert_eq!(result.declarations().len(), 17);
    assert!(matches!(
        result.declarations()[16].declaration(),
        PhoneDeclaration::Allophone { index: 7, .. }
    ));
    assert!(SpeechPhonemeDefinitionMatch::new(pid("foreign"), pid("phoneme/t")).is_err());
}

#[test]
fn named_environment_specs_keep_six_states_through_native_round_trips() {
    use conduit_plot::rust_binding::NativeRustBinding;
    let confidence = SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap();
    let syllables = [
        SpeechSyllablePositionSpecification::known(SpeechSyllablePosition::Coda).unwrap(),
        SpeechSyllablePositionSpecification::unknown(),
        SpeechSyllablePositionSpecification::unspecified(),
        SpeechSyllablePositionSpecification::not_applicable(),
        SpeechSyllablePositionSpecification::variable(
            BoundedSequence::try_from_iter([SpeechSyllablePosition::Coda]).unwrap(),
        )
        .unwrap(),
        SpeechSyllablePositionSpecification::gradient(
            confidence.clone(),
            SpeechSyllablePosition::Coda,
        )
        .unwrap(),
    ];
    let contexts = [
        SpeechProsodicContextSpecification::known(SpeechProsodicContext::CarefulSpeech).unwrap(),
        SpeechProsodicContextSpecification::unknown(),
        SpeechProsodicContextSpecification::unspecified(),
        SpeechProsodicContextSpecification::not_applicable(),
        SpeechProsodicContextSpecification::variable(
            BoundedSequence::try_from_iter([SpeechProsodicContext::CarefulSpeech]).unwrap(),
        )
        .unwrap(),
        SpeechProsodicContextSpecification::gradient(
            confidence,
            SpeechProsodicContext::CarefulSpeech,
        )
        .unwrap(),
    ];
    for (syllable, context) in syllables.into_iter().zip(contexts) {
        let environment = SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            context,
            StressSpecification::unknown(),
            syllable,
            SpeechPositionSpecification::unknown(),
        )
        .unwrap();
        let value = environment.clone().into_structured().unwrap();
        let decoded = SpeechEnvironment::from_structured(value).unwrap();
        assert_eq!(decoded, environment);
    }
}

#[test]
fn allophone_scalar_context_retains_rule_and_unresolved_observations() {
    use conduit_speech::allophone_context::*;
    let declaration = allophone("phone/t");
    let stress = StressSpecification::known(SpeechStress::Primary).unwrap();
    let word = SpeechPositionSpecification::unknown();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    let compared = compare_allophone_scalar_context(
        &declaration,
        ScalarContextObservation {
            stress: &stress,
            word_position: &word,
            syllable_position: &syllable,
            prosodic_context: &prosody,
        },
    )
    .unwrap();
    assert!(core::ptr::eq(compared.declaration(), &declaration));
    assert!(core::ptr::eq(
        compared.stress().requirement(),
        declaration.environment().stress_context()
    ));
    assert!(core::ptr::eq(compared.stress().observation(), &stress));
    assert_eq!(
        compared.stress().decision(),
        &SpeechContextDecision::RequirementUnresolved
    );
    assert_eq!(
        compared.word_position().decision(),
        &SpeechContextDecision::RequirementUnresolved
    );
    assert_eq!(
        compared.syllable_position().decision(),
        &SpeechContextDecision::RequirementUnresolved
    );
    assert_eq!(
        compared.prosodic_context().decision(),
        &SpeechContextDecision::RequirementUnresolved
    );
    assert_eq!(compared.declaration().conditions().as_slice().len(), 1);
}

#[test]
fn allophone_comparison_keeps_each_scalar_domain_separate() {
    use conduit_speech::allophone_context::*;
    let original = allophone("phone/t");
    let declaration = SpeechPhonemeAllophone::new(
        original.conditions().clone(),
        original.confidence().clone(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::known(SpeechStress::Primary).unwrap(),
            SpeechSyllablePositionSpecification::known(SpeechSyllablePosition::Onset).unwrap(),
            SpeechPositionSpecification::unknown(),
        )
        .unwrap(),
        original.phone().clone(),
        None,
        SpeechRuleStatus::Optional,
    )
    .unwrap();
    let stress = StressSpecification::known(SpeechStress::Secondary).unwrap();
    let word = SpeechPositionSpecification::known(SpeechWordPosition::Final).unwrap();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    let compared = compare_allophone_scalar_context(
        &declaration,
        ScalarContextObservation {
            stress: &stress,
            word_position: &word,
            syllable_position: &syllable,
            prosodic_context: &prosody,
        },
    )
    .unwrap();
    assert_eq!(
        compared.stress().decision(),
        &SpeechContextDecision::Mismatched
    );
    assert_eq!(
        compared.word_position().decision(),
        &SpeechContextDecision::RequirementUnresolved
    );
    assert_eq!(
        compared.syllable_position().decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert_eq!(
        compared.prosodic_context().decision(),
        &SpeechContextDecision::Matched
    );
}

#[test]
fn allophone_neighbor_receipt_keeps_original_rule_and_immediate_sides() {
    use conduit_speech::{allophone_context::*, neighbor_match::*};
    let original = allophone("phone/t");
    let declaration = SpeechPhonemeAllophone::new(
        original.conditions().clone(),
        original.confidence().clone(),
        SpeechEnvironment::new(
            BoundedSequence::try_from_iter([
                SpeechSegmentMatcher::phone(fixture::id("phone/t")).unwrap()
            ])
            .unwrap(),
            BoundedSequence::try_from_iter([
                SpeechSegmentMatcher::phone(fixture::id("phone/d")).unwrap()
            ])
            .unwrap(),
            SpeechProsodicContextSpecification::unknown(),
            StressSpecification::unknown(),
            SpeechSyllablePositionSpecification::unknown(),
            SpeechPositionSpecification::unknown(),
        )
        .unwrap(),
        original.phone().clone(),
        None,
        SpeechRuleStatus::Optional,
    )
    .unwrap();
    let phone = PhoneSpecification::known(fixture::id("phone/t")).unwrap();
    let phoneme = PhonemeSpecification::unknown();
    let observation = NeighborObservation::Segment {
        phone: &phone,
        phoneme: &phoneme,
        features: None,
    };
    let compared = compare_allophone_neighbors(&declaration, observation, observation).unwrap();
    assert!(core::ptr::eq(compared.declaration(), &declaration));
    assert_eq!(compared.before().decision(), &NeighborDecision::Mismatched);
    assert_eq!(compared.after().decision(), &NeighborDecision::Matched);
    assert!(core::ptr::eq(
        compared
            .before()
            .comparisons()
            .next()
            .unwrap()
            .requirement(),
        &declaration.environment().before().as_slice()[0]
    ));
}
