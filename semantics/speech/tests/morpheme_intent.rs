#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{morpheme_intent::*, semantic::*};
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new("explicit-test".into(), SpeechEvidenceSource::Manual, None)
        .unwrap()
}
fn basis(revision: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        SpeechSegmentRevisionId::new(revision.into()).unwrap(),
        SpeechSegmentSequenceId::new("target-phones".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn intent(revision: &str) -> SpeechUtteranceIntent {
    SpeechUtteranceIntent::new(
        BoundedSequence::new(),
        basis(revision).inventory_id().clone(),
        basis(revision).language().clone(),
        provenance(),
        basis(revision).revision_id().clone(),
        basis(revision).utterance_id().clone(),
    )
    .unwrap()
}
fn occurrence(ordinal: u32, revision: &str) -> LanguageSpeechTokenRef {
    let b = basis(revision);
    LanguageSpeechTokenRef::new(
        b.inventory_id().clone(),
        b.language().clone(),
        ordinal,
        b.revision_id().clone(),
        b.sequence_id().clone(),
        b.utterance_id().clone(),
    )
    .unwrap()
}

fn phonemes() -> SpeechPhonemeSequence {
    let token = SpeechPhonemeToken::new(
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeSpecification::unknown(),
        provenance(),
        BoundedSequence::new(),
        None,
    )
    .unwrap();
    SpeechPhonemeSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter([token.clone(), token]).unwrap(),
    )
    .unwrap()
}
fn morpheme(
    refs: &[LanguageSpeechTokenRef],
    source: Option<LanguageMorphemeReference>,
    spec: SpeechMorphemeSpecification,
) -> SpeechPlannedMorphemeIntent {
    SpeechPlannedMorphemeIntent::new(
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        spec,
        BoundedSequence::try_from_iter(refs.iter().cloned()).unwrap(),
        provenance(),
        source,
        "café".into(),
    )
    .unwrap()
}
fn source(end: u32, revision: &str) -> LanguageMorphemeReference {
    LanguageMorphemeReference::new(
        LanguageTextSegmentRef::new(
            LanguageTextSegmentKind::Morpheme,
            LanguageId::new("fr".into()).unwrap(),
            LanguageTextRange::new(end, 0).unwrap(),
            LanguageTextRevisionId::new(revision.into()).unwrap(),
            LanguageTextId::new("source".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn text(value: &str) -> LanguageText {
    LanguageText::new(
        LanguageTextId::new("source".into()).unwrap(),
        LanguageId::new("fr".into()).unwrap(),
        LanguageTextRevisionId::new("source-r1".into()).unwrap(),
        value.into(),
    )
    .unwrap()
}
#[test]
fn scalar_source_and_foreign_language_custody() {
    let i = intent("r1");
    let p = phonemes();
    let t = text("café");
    let m = morpheme(
        &[occurrence(0, "r1")],
        Some(source(4, "source-r1")),
        SpeechMorphemeSpecification::unknown(),
    );
    let prepared = PreparedMorphemeIntent::prepare(&i, &p, &m, Some(&t)).unwrap();
    assert!(core::ptr::eq(prepared.intent(), &i));
    assert!(core::ptr::eq(prepared.phonemes(), &p));
    assert!(core::ptr::eq(prepared.text().unwrap(), &t));
    assert!(prepared.surface_match().is_some());
    for bad in [source(5, "source-r1"), source(4, "stale")] {
        let m = morpheme(&[], Some(bad), SpeechMorphemeSpecification::unknown());
        assert!(PreparedMorphemeIntent::prepare(&i, &p, &m, Some(&t)).is_err());
    }
    let changed = text("cafe");
    assert!(PreparedMorphemeIntent::prepare(&i, &p, &m, Some(&changed)).is_err());
    assert!(PreparedMorphemeIntent::prepare(&i, &p, &m, None).is_err());
}
#[test]
fn pronunciation_refuses_stale_missing_reversed_duplicate_occurrences() {
    let i = intent("r1");
    let p = phonemes();
    for refs in [
        vec![occurrence(0, "stale")],
        vec![occurrence(2, "r1")],
        vec![occurrence(1, "r1"), occurrence(0, "r1")],
        vec![occurrence(0, "r1"), occurrence(0, "r1")],
    ] {
        let m = morpheme(&refs, None, SpeechMorphemeSpecification::unknown());
        assert!(PreparedMorphemeIntent::prepare(&i, &p, &m, None).is_err());
    }
    let m = morpheme(&[], None, SpeechMorphemeSpecification::unspecified());
    let result = PreparedMorphemeIntent::prepare(&i, &p, &m, None).unwrap();
    assert!(result.members().is_empty());
    assert!(result.source_match().is_none());
    assert!(PreparedMorphemeIntent::prepare(&i, &p, &m, Some(&text("café"))).is_err());
}
#[test]
fn six_specification_states_survive_without_inference() {
    let id = LanguageMorphemeId::new("morpheme".into()).unwrap();
    let states = [
        SpeechMorphemeSpecification::known(id.clone()).unwrap(),
        SpeechMorphemeSpecification::unknown(),
        SpeechMorphemeSpecification::unspecified(),
        SpeechMorphemeSpecification::not_applicable(),
        SpeechMorphemeSpecification::variable(
            BoundedSequence::try_from_iter([id.clone()]).unwrap(),
        )
        .unwrap(),
        SpeechMorphemeSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            id,
        )
        .unwrap(),
    ];
    let i = intent("r1");
    let p = phonemes();
    for state in states {
        let m = morpheme(&[occurrence(0, "r1"), occurrence(1, "r1")], None, state);
        let encoded = m.clone().into_structured().unwrap();
        let roundtrip = SpeechPlannedMorphemeIntent::from_structured(encoded).unwrap();
        assert_eq!(m, roundtrip);
        let prepared = PreparedMorphemeIntent::prepare(&i, &p, &m, None).unwrap();
        assert!(core::ptr::eq(prepared.morpheme(), &m));
        assert_eq!(prepared.order().len(), 1);
        assert_eq!(prepared.references().len(), 2);
    }
}
