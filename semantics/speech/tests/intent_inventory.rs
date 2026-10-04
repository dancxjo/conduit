#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{intent_inventory::*, semantic::*};
#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod fixture;
use fixture::{definition, id, inventory};
fn occurrence(sequence: &str, ordinal: u32) -> LanguageSpeechTokenRef {
    LanguageSpeechTokenRef::new(
        SpeechInventoryId::new("english-test".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        ordinal,
        SpeechSegmentRevisionId::new("revision-2".into()).unwrap(),
        SpeechSegmentSequenceId::new(sequence.into()).unwrap(),
        SpeechUtteranceId::new("utterance-1".into()).unwrap(),
    )
    .unwrap()
}
fn intent(occurrence: LanguageSpeechTokenRef, phone: PhoneSpecification) -> SpeechUtteranceIntent {
    use conduit_plot::rust_binding::BoundedSequence;
    let provenance = SpeechEvidenceProvenance::new(
        "membership-fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap();
    // Source provenance intentionally names another revision and inventory.
    let source = LanguageSegmentRef::phone(
        SpeechInventoryId::new("source-inventory".into()).unwrap(),
        SpeechLanguageId::new("es".into()).unwrap(),
        77,
        SpeechSegmentRevisionId::new("source-revision".into()).unwrap(),
        SpeechSegmentSequenceId::new("source-phones".into()).unwrap(),
        SpeechUtteranceId::new("source-utterance".into()).unwrap(),
    )
    .unwrap();
    let sources = BoundedSequence::try_from_iter([source]).unwrap();
    let boundary = SpeechUtteranceIntentEvent::boundary(
        SpeechDurationSpecification::unknown(),
        SpeechBoundarySpecification::unspecified(),
        provenance.clone(),
        sources.clone(),
    )
    .unwrap();
    let segment = SpeechUtteranceIntentEvent::segment(
        occurrence,
        phone,
        PhonemeSpecification::unspecified(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::unknown(),
            SpeechCycleSpecification::unspecified(),
            SpeechIntensitySpecification::not_applicable(),
        )
        .unwrap(),
        provenance.clone(),
        sources,
        StressSpecification::unknown(),
        SpeechPositionSpecification::unspecified(),
    )
    .unwrap();
    SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter([boundary, segment]).unwrap(),
        SpeechInventoryId::new("english-test".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        provenance,
        SpeechSegmentRevisionId::new("revision-2".into()).unwrap(),
        SpeechUtteranceId::new("utterance-1".into()).unwrap(),
    )
    .unwrap()
}

#[test]
fn lookup_borrows_exact_intent_and_definition() {
    let intent = intent(
        occurrence("phones", 91),
        PhoneSpecification::known(id("opaque/t")).unwrap(),
    );
    let inventory = inventory("english-test", "en", vec![definition("opaque/t")]);
    let found = resolve_intent_inventory_phone(&intent, 1, &inventory).unwrap();
    assert!(core::ptr::eq(found.intent(), &intent));
    assert!(core::ptr::eq(found.inventory(), &inventory));
    assert!(core::ptr::eq(
        found.definition(),
        &inventory.phones().as_slice()[0]
    ));
    let SpeechUtteranceIntentEvent::Segment(segment) = &intent.events().as_slice()[1] else {
        panic!()
    };
    assert!(core::ptr::eq(found.segment(), segment));
    assert_eq!(found.event(), 1);
    assert_eq!(
        found.checked_occurrence().occurrence(),
        segment.occurrence()
    );
    assert_eq!(
        found.checked_identity().requested(),
        found.definition().identity()
    );
    assert_eq!(found.checked_basis().intent_language(), intent.language());
}
#[test]
fn states_remain_unresolved() {
    let inventory = inventory("english-test", "en", vec![definition("opaque/t")]);
    for state in [
        PhoneSpecification::unknown(),
        PhoneSpecification::unspecified(),
        PhoneSpecification::not_applicable(),
        PhoneSpecification::variable(BoundedSequence::try_from_iter([id("opaque/t")]).unwrap())
            .unwrap(),
        PhoneSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
            id("opaque/t"),
        )
        .unwrap(),
    ] {
        let intent = intent(occurrence("phones", 0), state.clone());
        match resolve_intent_inventory_phone(&intent, 1, &inventory) {
            Err(IntentInventoryRefusal::Unresolved(actual)) => assert_eq!(actual, state),
            _ => panic!("must preserve specification"),
        }
    }
}
#[test]
fn event_basis_and_definition_failures_are_distinct() {
    let value = intent(
        occurrence("phones", 0),
        PhoneSpecification::known(id("opaque/t")).unwrap(),
    );
    let valid = inventory("english-test", "en", vec![definition("opaque/t")]);
    assert!(matches!(
        resolve_intent_inventory_phone(&value, 0, &valid),
        Err(IntentInventoryRefusal::BoundaryEvent)
    ));
    assert!(matches!(
        resolve_intent_inventory_phone(&value, 2, &valid),
        Err(IntentInventoryRefusal::MissingEvent)
    ));
    for (identity, language) in [("other", "en"), ("english-test", "es")] {
        let foreign = inventory(identity, language, vec![definition("opaque/t")]);
        assert!(matches!(
            resolve_intent_inventory_phone(&value, 1, &foreign),
            Err(IntentInventoryRefusal::Basis(_))
        ));
    }
    let absent = inventory("english-test", "en", vec![definition("other/t")]);
    assert!(matches!(
        resolve_intent_inventory_phone(&value, 1, &absent),
        Err(IntentInventoryRefusal::MissingDefinition)
    ));
    let duplicate = inventory(
        "english-test",
        "en",
        vec![definition("opaque/t"), definition("opaque/t")],
    );
    assert!(matches!(
        resolve_intent_inventory_phone(&value, 1, &duplicate),
        Err(IntentInventoryRefusal::AmbiguousDefinition)
    ));
    let foreign = LanguageSpeechTokenRef::new(
        SpeechInventoryId::new("other".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        0,
        SpeechSegmentRevisionId::new("revision-2".into()).unwrap(),
        SpeechSegmentSequenceId::new("phones".into()).unwrap(),
        SpeechUtteranceId::new("utterance-1".into()).unwrap(),
    )
    .unwrap();
    let foreign = intent(foreign, PhoneSpecification::known(id("opaque/t")).unwrap());
    assert!(matches!(
        resolve_intent_inventory_phone(&foreign, 1, &valid),
        Err(IntentInventoryRefusal::Occurrence(_))
    ));
}
