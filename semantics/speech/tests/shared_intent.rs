#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{semantic::*, shared_intent::*};
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

fn phone() -> SpeechPhoneToken {
    SpeechPhoneToken::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneSpecification::unknown(),
        provenance(),
        None,
    )
    .unwrap()
}
fn phoneme(realized: &[SpeechPhoneToken]) -> SpeechPhonemeToken {
    SpeechPhonemeToken::new(
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeSpecification::unknown(),
        provenance(),
        BoundedSequence::try_from_iter(realized.iter().cloned()).unwrap(),
        None,
    )
    .unwrap()
}
fn group(
    p: &[LanguageSpeechTokenRef],
    q: &[LanguageSpeechTokenRef],
    kind: SpeechRealizationCorrespondenceKind,
) -> Result<SpeechPhonemePhoneCorrespondence, conduit_plot::rust_binding::NativeBindingRefusal> {
    SpeechPhonemePhoneCorrespondence::new(
        kind,
        BoundedSequence::try_from_iter(p.iter().cloned()).unwrap(),
        BoundedSequence::try_from_iter(q.iter().cloned()).unwrap(),
        provenance(),
    )
}

fn context() -> SpeechUtteranceIntentContext {
    SpeechUtteranceIntentContext::new(
        None,
        provenance(),
        SpeechReferenceProjectionCapability::new("retained_metadata_only".into()).unwrap(),
        SpeechSpeakerReferenceSpecification::unknown(),
        SpeechStyleReferenceSpecification::unspecified(),
        LanguageVariety::new(
            VarietyId::new("declared".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn morph() -> SpeechPlannedMorphemeIntent {
    SpeechPlannedMorphemeIntent::new(
        SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        SpeechMorphemeSpecification::unknown(),
        BoundedSequence::try_from_iter([occurrence(0, "r1")]).unwrap(),
        provenance(),
        None,
        "a".into(),
    )
    .unwrap()
}
fn syllable() -> SpeechPlannedSyllableIntent {
    SpeechPlannedSyllableIntent::new(
        basis("r1"),
        SpeechSyllableId::new("syllable".into()).unwrap(),
        Some(0),
        BoundedSequence::try_from_iter([SpeechSyllablePosition::Nucleus]).unwrap(),
        BoundedSequence::try_from_iter([occurrence(0, "r1")]).unwrap(),
        provenance(),
        None,
        StressSpecification::known(SpeechStress::Primary).unwrap(),
    )
    .unwrap()
}
#[test]
fn composed_owner_retains_exact_original_and_every_component_receipt() {
    let original = intent("r1");
    let c = context();
    let actual = phone();
    let p = SpeechPhonemeSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter([phoneme(&[actual.clone()])]).unwrap(),
    )
    .unwrap();
    let q = SpeechPhoneSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter([actual]).unwrap(),
    )
    .unwrap();
    let m = [morph()];
    let texts = [None];
    let s = [syllable()];
    let corr = [group(
        &[occurrence(0, "r1")],
        &[occurrence(0, "r1")],
        SpeechRealizationCorrespondenceKind::Realized,
    )
    .unwrap()];
    let components = SpeechIntentComponents {
        context: &c,
        intended_text: None,
        phonemes: &p,
        phones: &q,
        morphemes: &m,
        morpheme_texts: &texts,
        syllables: &s,
        correspondences: &corr,
    };
    let prepared = PreparedSpeechUtteranceIntent::prepare(&original, components).unwrap();
    assert!(core::ptr::eq(prepared.original(), &original));
    assert!(core::ptr::eq(prepared.components().phonemes, &p));
    assert!(core::ptr::eq(prepared.components().phones, &q));
    assert!(core::ptr::eq(prepared.context().context(), &c));
    assert!(core::ptr::eq(prepared.morphemes()[0].morpheme(), &m[0]));
    assert!(core::ptr::eq(prepared.syllables()[0].syllable(), &s[0]));
    assert!(core::ptr::eq(
        prepared.correspondences()[0].correspondence(),
        &corr[0]
    ));
    assert_eq!(*prepared.counts().morphemes(), 1);
}
#[test]
fn missing_material_count_foreign_basis_and_over_profile_refuse_composition() {
    let original = intent("r1");
    let c = context();
    let p = SpeechPhonemeSequence::new(basis("r1"), BoundedSequence::new()).unwrap();
    let q = SpeechPhoneSequence::new(basis("r1"), BoundedSequence::new()).unwrap();
    let m = [morph()];
    let components = SpeechIntentComponents {
        context: &c,
        intended_text: None,
        phonemes: &p,
        phones: &q,
        morphemes: &m,
        morpheme_texts: &[],
        syllables: &[],
        correspondences: &[],
    };
    assert!(PreparedSpeechUtteranceIntent::prepare(&original, components).is_err());
    let over = vec![morph(); 33];
    let texts = vec![None; 33];
    let components = SpeechIntentComponents {
        context: &c,
        intended_text: None,
        phonemes: &p,
        phones: &q,
        morphemes: &over,
        morpheme_texts: &texts,
        syllables: &[],
        correspondences: &[],
    };
    assert!(PreparedSpeechUtteranceIntent::prepare(&original, components).is_err());
    let foreign = SpeechPhoneSequence::new(basis("foreign"), BoundedSequence::new()).unwrap();
    let components = SpeechIntentComponents {
        context: &c,
        intended_text: None,
        phonemes: &p,
        phones: &foreign,
        morphemes: &[],
        morpheme_texts: &[],
        syllables: &[],
        correspondences: &[],
    };
    assert!(PreparedSpeechUtteranceIntent::prepare(&original, components).is_err());
}
