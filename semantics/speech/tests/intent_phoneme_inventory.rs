#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{intent_phoneme_inventory::*, semantic::*};
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod occurrence;
fn definition(id: &str) -> SpeechPhoneme {
    SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::new(),
        None,
        SpeechFeatureBundle::new(
            BoundedSequence::try_from_iter([SpeechFeature::new(
                SpeechFeatureId::new("voice".into()).unwrap(),
                FeatureSpecification::unknown(),
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap(),
        PhonemeId::new(id.into()).unwrap(),
        "t".into(),
        BoundedSequence::new(),
        SpeechSegmentStatus::Core,
    )
    .unwrap()
}
fn inventory(id: &str, language: &str, definitions: Vec<SpeechPhoneme>) -> SpeechInventory {
    SpeechInventory::new(
        SpeechInventoryId::new(id.into()).unwrap(),
        SpeechLanguageId::new(language.into()).unwrap(),
        BoundedSequence::try_from_iter(definitions).unwrap(),
        BoundedSequence::new(),
    )
    .unwrap()
}
fn intent(specification: PhonemeSpecification) -> SpeechUtteranceIntent {
    let SpeechUtteranceIntentEvent::Segment(original) = occurrence::segment(10) else {
        unreachable!()
    };
    occurrence::intent([SpeechUtteranceIntentEvent::segment(
        original.occurrence().clone(),
        original.phone().clone(),
        specification,
        original.prosody().clone(),
        original.provenance().clone(),
        original.sources().clone(),
        original.stress().clone(),
        original.word_position().clone(),
    )
    .unwrap()])
}
fn known(id: &str) -> PhonemeSpecification {
    PhonemeSpecification::known(PhonemeId::new(id.into()).unwrap()).unwrap()
}
#[test]
fn original_occurrence_and_complete_phoneme_definition_are_retained_without_feature_inference() {
    let intent = intent(known("phoneme/t"));
    let inventory = inventory("inventory", "en", vec![definition("phoneme/t")]);
    let before = intent.clone();
    let resolved = resolve_intent_inventory_phoneme(&intent, 0, &inventory).unwrap();
    assert!(core::ptr::eq(resolved.inventory(), &inventory));
    assert!(core::ptr::eq(
        resolved.definition(),
        &inventory.phonemes().as_slice()[0]
    ));
    let SpeechUtteranceIntentEvent::Segment(original) = &intent.events().as_slice()[0] else {
        unreachable!()
    };
    assert!(core::ptr::eq(resolved.occurrence().segment(), original));
    assert_eq!(
        resolved.checked_identity().definition(),
        resolved.definition().identity()
    );
    assert_eq!(
        resolved.checked_identity().requested(),
        &PhonemeId::new("phoneme/t".into()).unwrap()
    );
    assert_eq!(
        resolved.definition().features().get().as_slice()[0].specification(),
        &FeatureSpecification::unknown()
    );
    assert_eq!(intent, before);
}
#[test]
fn all_five_nonknown_states_refuse_with_the_original_specification() {
    let id = PhonemeId::new("phoneme/t".into()).unwrap();
    let inventory = inventory("inventory", "en", vec![definition("phoneme/t")]);
    for state in [
        PhonemeSpecification::unknown(),
        PhonemeSpecification::unspecified(),
        PhonemeSpecification::not_applicable(),
        PhonemeSpecification::variable(BoundedSequence::try_from_iter([id.clone()]).unwrap())
            .unwrap(),
        PhonemeSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            id,
        )
        .unwrap(),
    ] {
        let intent = intent(state);
        let SpeechUtteranceIntentEvent::Segment(original) = &intent.events().as_slice()[0] else {
            unreachable!()
        };
        assert!(
            matches!(resolve_intent_inventory_phoneme(&intent, 0, &inventory), Err(IntentPhonemeRefusal::Unresolved(value)) if core::ptr::eq(value, original.phoneme()))
        );
    }
}
#[test]
fn missing_duplicate_case_and_unicode_identities_have_no_notation_or_base_fallback() {
    let exact = intent(known("phoneme/t"));
    let missing = inventory("inventory", "en", vec![definition("phoneme/other")]);
    assert!(matches!(
        resolve_intent_inventory_phoneme(&exact, 0, &missing),
        Err(IntentPhonemeRefusal::MissingDefinition)
    ));
    let duplicate = inventory(
        "inventory",
        "en",
        vec![definition("phoneme/t"), definition("phoneme/t")],
    );
    assert!(matches!(
        resolve_intent_inventory_phoneme(&exact, 0, &duplicate),
        Err(IntentPhonemeRefusal::AmbiguousDefinition)
    ));
    for (requested, declared) in [
        ("phoneme/T", "phoneme/t"),
        ("phoneme/t/stressed", "phoneme/t"),
        ("phoneme/é", "phoneme/e\u{301}"),
    ] {
        let intent = intent(known(requested));
        let inventory = inventory("inventory", "en", vec![definition(declared)]);
        assert!(matches!(
            resolve_intent_inventory_phoneme(&intent, 0, &inventory),
            Err(IntentPhonemeRefusal::MissingDefinition)
        ));
    }
}
#[test]
fn foreign_inventory_basis_and_invalid_event_context_refuse_before_lookup() {
    let intent = intent(known("phoneme/t"));
    for (id, language) in [("foreign", "en"), ("inventory", "foreign")] {
        let inventory = inventory(id, language, vec![definition("phoneme/t")]);
        assert!(matches!(
            resolve_intent_inventory_phoneme(&intent, 0, &inventory),
            Err(IntentPhonemeRefusal::Basis(_))
        ));
    }
    let inventory = inventory("inventory", "en", vec![definition("phoneme/t")]);
    assert!(matches!(
        resolve_intent_inventory_phoneme(&intent, 1, &inventory),
        Err(IntentPhonemeRefusal::Occurrence(_))
    ));
    let boundary = occurrence::intent([occurrence::boundary(
        SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
    )]);
    assert!(matches!(
        resolve_intent_inventory_phoneme(&boundary, 0, &inventory),
        Err(IntentPhonemeRefusal::Occurrence(_))
    ));
    let gap = occurrence::intent([occurrence::segment(10), occurrence::segment(12)]);
    assert!(matches!(
        resolve_intent_inventory_phoneme(&gap, 0, &inventory),
        Err(IntentPhonemeRefusal::Occurrence(_))
    ));
}
