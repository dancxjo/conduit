#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::semantic::*;

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
fn membership(
    value: LanguageSpeechTokenRef,
    inventory: &str,
    language: &str,
    revision: &str,
    utterance: &str,
) -> Result<SpeechOccurrenceMembership, conduit_plot::rust_binding::NativeBindingRefusal> {
    SpeechOccurrenceMembership::new(
        SpeechInventoryId::new(inventory.into()).unwrap(),
        SpeechLanguageId::new(language.into()).unwrap(),
        value,
        SpeechSegmentRevisionId::new(revision.into()).unwrap(),
        SpeechUtteranceId::new(utterance.into()).unwrap(),
    )
}
#[test]
fn requested_membership_preserves_sequence_and_ordinal_and_roundtrips() {
    for (sequence, ordinal) in [("phonemes", 0), ("phones", 91)] {
        let occurrence = occurrence(sequence, ordinal);
        let receipt = membership(
            occurrence.clone(),
            "english-test",
            "en",
            "revision-2",
            "utterance-1",
        )
        .unwrap();
        assert_eq!(receipt.occurrence(), &occurrence);
        assert_eq!(
            SpeechOccurrenceMembership::from_structured(receipt.clone().into_structured().unwrap())
                .unwrap(),
            receipt
        );
    }
}
#[test]
fn same_ordinal_cannot_authorize_another_revision_or_inventory() {
    for (inventory, language, revision, utterance) in [
        ("other-inventory", "en", "revision-2", "utterance-1"),
        ("english-test", "es", "revision-2", "utterance-1"),
        ("english-test", "en", "revision-1", "utterance-1"),
        ("english-test", "en", "revision-2", "other-utterance"),
    ] {
        assert!(membership(
            occurrence("phones", 0),
            inventory,
            language,
            revision,
            utterance
        )
        .is_err());
    }
}

#[test]
fn structurally_valid_foreign_revision_is_refused_on_decode() {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};
    let receipt = membership(
        occurrence("phones", 0),
        "english-test",
        "en",
        "revision-2",
        "utterance-1",
    )
    .unwrap();
    let value = receipt.into_structured().unwrap();
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record expected")
    };
    let foreign = SpeechSegmentRevisionId::new("revision-1".into())
        .unwrap()
        .into_structured()
        .unwrap();
    let fields = fields
        .iter()
        .map(|field| {
            StructuredFieldValue::new(
                field.name(),
                if field.name() == "revision_id" {
                    foreign.clone()
                } else {
                    field.value().clone()
                },
            )
            .unwrap()
        })
        .collect();
    let forged = StructuredInfoValue::record(value.value_type().clone(), fields).unwrap();
    assert!(SpeechOccurrenceMembership::from_structured(forged).is_err());
}

fn intent(occurrence: LanguageSpeechTokenRef) -> SpeechUtteranceIntent {
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
        PhoneSpecification::unknown(),
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
fn intent_checks_its_occurrences_without_resolving_specs_or_rebinding_sources() {
    use conduit_speech::intent_admission::validate_intent_occurrences;
    let value = intent(occurrence("phones", 0));
    validate_intent_occurrences(&value).unwrap();
    let foreign = LanguageSpeechTokenRef::new(
        SpeechInventoryId::new("english-test".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        0,
        SpeechSegmentRevisionId::new("revision-1".into()).unwrap(),
        SpeechSegmentSequenceId::new("phones".into()).unwrap(),
        SpeechUtteranceId::new("utterance-1".into()).unwrap(),
    )
    .unwrap();
    let refusal = validate_intent_occurrences(&intent(foreign)).unwrap_err();
    assert_eq!(refusal.event, 1);
}
