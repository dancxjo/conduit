#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal, NativeRustBinding};
use conduit_speech::{reference_admission::*, semantic::*};

fn basis() -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechSegmentSequenceId::new("sequence".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn reference(phone: bool, ordinal: u32, revision: &str, sequence: &str) -> LanguageSegmentRef {
    let inventory = SpeechInventoryId::new("inventory".into()).unwrap();
    let language = SpeechLanguageId::new("en".into()).unwrap();
    let revision = SpeechSegmentRevisionId::new(revision.into()).unwrap();
    let sequence = SpeechSegmentSequenceId::new(sequence.into()).unwrap();
    let utterance = SpeechUtteranceId::new("utterance".into()).unwrap();
    if phone {
        LanguageSegmentRef::phone(inventory, language, ordinal, revision, sequence, utterance)
            .unwrap()
    } else {
        LanguageSegmentRef::phoneme(inventory, language, ordinal, revision, sequence, utterance)
            .unwrap()
    }
}
fn phone(specification: PhoneSpecification) -> SpeechPhoneToken {
    SpeechPhoneToken::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        specification,
        SpeechEvidenceProvenance::new("fixture".into(), SpeechEvidenceSource::Manual, None)
            .unwrap(),
        None,
    )
    .unwrap()
}
fn phones(tokens: impl IntoIterator<Item = SpeechPhoneToken>) -> SpeechPhoneSequence {
    SpeechPhoneSequence::new(basis(), BoundedSequence::try_from_iter(tokens).unwrap()).unwrap()
}
#[test]
fn phone_resolution_borrows_material_without_selecting_specifications() {
    let id = PhoneId::new("en/t".into()).unwrap();
    let states = [
        PhoneSpecification::known(id.clone()).unwrap(),
        PhoneSpecification::unknown(),
        PhoneSpecification::unspecified(),
        PhoneSpecification::not_applicable(),
        PhoneSpecification::variable(BoundedSequence::try_from_iter([id.clone()]).unwrap())
            .unwrap(),
        PhoneSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
            id,
        )
        .unwrap(),
    ];
    let snapshot = phones(states.iter().cloned().map(phone));
    for (ordinal, state) in states.iter().enumerate() {
        let reference = reference(true, ordinal as u32, "revision", "sequence");
        let resolved = resolve_phone(&reference, &snapshot).unwrap();
        assert!(core::ptr::eq(
            resolved.token(),
            &snapshot.tokens().as_slice()[ordinal]
        ));
        assert!(core::ptr::eq(resolved.snapshot(), &snapshot));
        assert_eq!(resolved.reference(), &reference);
        assert_eq!(resolved.token().phone(), state);
        assert_eq!(*resolved.checked().token_count(), 6);
    }
}
#[test]
fn stale_revision_sequence_and_missing_ordinal_are_distinct_native_laws() {
    let snapshot = phones([phone(PhoneSpecification::unknown())]);
    for (ordinal, revision, sequence, basis_failure) in [
        (0, "stale", "sequence", true),
        (0, "revision", "other", true),
        (1, "revision", "sequence", false),
        (u32::MAX, "revision", "sequence", false),
    ] {
        let reference = reference(true, ordinal, revision, sequence);
        let failure = resolve_phone(&reference, &snapshot).err().unwrap();
        if basis_failure {
            assert!(matches!(
                failure,
                ReferenceRefusal::Basis(NativeBindingRefusal::ViolatedInvariant { .. })
            ));
        } else {
            assert!(matches!(
                failure,
                ReferenceRefusal::Ordinal(NativeBindingRefusal::ViolatedInvariant { .. })
            ));
        }
    }
    let empty = phones([]);
    assert!(matches!(
        resolve_phone(&reference(true, 0, "revision", "sequence"), &empty),
        Err(ReferenceRefusal::Ordinal(
            NativeBindingRefusal::ViolatedInvariant { .. }
        ))
    ));
}
#[test]
fn phonemes_remain_phonemes_and_retain_their_realization_evidence() {
    let realized = phone(PhoneSpecification::unknown());
    let token = SpeechPhonemeToken::new(
        SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeSpecification::unknown(),
        SpeechEvidenceProvenance::new(
            "phoneme-fixture".into(),
            SpeechEvidenceSource::ImportedData,
            None,
        )
        .unwrap(),
        BoundedSequence::try_from_iter([realized.clone()]).unwrap(),
        None,
    )
    .unwrap();
    let snapshot = SpeechPhonemeSequence::new(
        basis(),
        BoundedSequence::try_from_iter([token.clone()]).unwrap(),
    )
    .unwrap();
    let reference = reference(false, 0, "revision", "sequence");
    let resolved = resolve_phoneme(&reference, &snapshot).unwrap();
    assert!(core::ptr::eq(
        resolved.token(),
        &snapshot.tokens().as_slice()[0]
    ));
    assert_eq!(resolved.token(), &token);
    assert_eq!(resolved.token().realized_as().as_slice(), &[realized]);
    let phone_snapshot = phones([phone(PhoneSpecification::unknown())]);
    assert!(matches!(
        resolve_phone(&reference, &phone_snapshot),
        Err(ReferenceRefusal::ReferenceKind)
    ));
    assert!(matches!(
        resolve_phoneme(&self::reference(true, 0, "revision", "sequence"), &snapshot),
        Err(ReferenceRefusal::ReferenceKind)
    ));
    assert_ne!(
        snapshot.into_structured().unwrap().value_type(),
        phone_snapshot.into_structured().unwrap().value_type()
    );
}
#[test]
fn match_receipts_check_all_basis_fields_and_material_counts() {
    let reference = LanguageSpeechTokenRef::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        0,
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechSegmentSequenceId::new("sequence".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap();
    let original = basis();
    let valid = SpeechSequenceReferenceMatch::new(
        SpeechSequenceBasisMatch::new(original.clone(), reference.clone()).unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(
        SpeechSequenceReferenceMatch::from_structured(valid.clone().into_structured().unwrap())
            .unwrap(),
        valid
    );
    for (inventory, language, revision, sequence, utterance) in [
        ("foreign", "en", "revision", "sequence", "utterance"),
        ("inventory", "es", "revision", "sequence", "utterance"),
        ("inventory", "en", "foreign", "sequence", "utterance"),
        ("inventory", "en", "revision", "foreign", "utterance"),
        ("inventory", "en", "revision", "sequence", "foreign"),
    ] {
        let basis = SpeechTokenSequenceBasis::new(
            SpeechInventoryId::new(inventory.into()).unwrap(),
            SpeechLanguageId::new(language.into()).unwrap(),
            SpeechSegmentRevisionId::new(revision.into()).unwrap(),
            SpeechSegmentSequenceId::new(sequence.into()).unwrap(),
            SpeechUtteranceId::new(utterance.into()).unwrap(),
        )
        .unwrap();
        assert!(SpeechSequenceBasisMatch::new(basis, reference.clone()).is_err());
    }
    assert!(SpeechSequenceReferenceMatch::new(
        SpeechSequenceBasisMatch::new(original, reference).unwrap(),
        257
    )
    .is_err());
    assert!(BoundedSequence::<SpeechPhoneToken, 256>::try_from_iter(
        (0..257).map(|_| phone(PhoneSpecification::unknown()))
    )
    .is_err());
}
