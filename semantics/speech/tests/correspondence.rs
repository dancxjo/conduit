#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{correspondence::*, semantic::*};
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
#[test]
fn shared_realization_retains_both_complete_original_sequences() {
    let i = intent("r1");
    let actual = phone();
    let phonemes = SpeechPhonemeSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter([
            phoneme(core::slice::from_ref(&actual)),
            phoneme(core::slice::from_ref(&actual)),
        ])
        .unwrap(),
    )
    .unwrap();
    let phones = SpeechPhoneSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter([actual]).unwrap(),
    )
    .unwrap();
    let mapping = group(
        &[occurrence(0, "r1"), occurrence(1, "r1")],
        &[occurrence(0, "r1")],
        SpeechRealizationCorrespondenceKind::Realized,
    )
    .unwrap();
    let frame = mapping.clone().into_structured().unwrap();
    assert_eq!(
        SpeechPhonemePhoneCorrespondence::from_structured(frame).unwrap(),
        mapping
    );
    let p = PreparedPhonemePhoneCorrespondence::prepare(&i, &phonemes, &phones, &mapping).unwrap();
    assert!(core::ptr::eq(p.phonemes(), &phonemes));
    assert!(core::ptr::eq(p.phones(), &phones));
    assert!(core::ptr::eq(p.correspondence(), &mapping));
    assert_eq!(p.realization().len(), 2);
    let contradictory = SpeechPhonemeSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter([phoneme(&[]), phoneme(&[])]).unwrap(),
    )
    .unwrap();
    assert!(
        PreparedPhonemePhoneCorrespondence::prepare(&i, &contradictory, &phones, &mapping).is_err()
    );
}
#[test]
fn explicit_omission_insertion_unresolved_and_refusals() {
    let i = intent("r1");
    let phonemes = SpeechPhonemeSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter([phoneme(&[])]).unwrap(),
    )
    .unwrap();
    let phones = SpeechPhoneSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter([phone()]).unwrap(),
    )
    .unwrap();
    for (p, q, kind) in [
        (
            vec![occurrence(0, "r1")],
            vec![],
            SpeechRealizationCorrespondenceKind::Omitted,
        ),
        (
            vec![],
            vec![occurrence(0, "r1")],
            SpeechRealizationCorrespondenceKind::Inserted,
        ),
        (
            vec![occurrence(0, "r1")],
            vec![occurrence(0, "r1")],
            SpeechRealizationCorrespondenceKind::Unresolved,
        ),
    ] {
        let m = group(&p, &q, kind).unwrap();
        PreparedPhonemePhoneCorrespondence::prepare(&i, &phonemes, &phones, &m).unwrap();
    }
    assert!(group(&[], &[], SpeechRealizationCorrespondenceKind::Unresolved).is_err());
    assert!(group(
        &[occurrence(0, "r1")],
        &[],
        SpeechRealizationCorrespondenceKind::Realized
    )
    .is_err());
    for refs in [
        vec![occurrence(0, "stale")],
        vec![occurrence(9, "r1")],
        vec![occurrence(0, "r1"), occurrence(0, "r1")],
    ] {
        let m = group(&refs, &[], SpeechRealizationCorrespondenceKind::Unresolved).unwrap();
        assert!(PreparedPhonemePhoneCorrespondence::prepare(&i, &phonemes, &phones, &m).is_err());
    }
}
#[test]
fn many_to_many_keeps_distinct_phoneme_and_phone_sequence_identities() {
    let i = intent("r1");
    let a = phone();
    let b = SpeechPhoneToken::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneSpecification::known(PhoneId::new("other-phone".into()).unwrap()).unwrap(),
        provenance(),
        None,
    )
    .unwrap();
    let old = basis("r1");
    let intended_basis = SpeechTokenSequenceBasis::new(
        old.inventory_id().clone(),
        old.language().clone(),
        old.revision_id().clone(),
        SpeechSegmentSequenceId::new("intended-phonemes".into()).unwrap(),
        old.utterance_id().clone(),
    )
    .unwrap();
    let p = SpeechPhonemeSequence::new(
        intended_basis.clone(),
        BoundedSequence::try_from_iter([
            phoneme(&[a.clone(), b.clone()]),
            phoneme(&[a.clone(), b.clone()]),
        ])
        .unwrap(),
    )
    .unwrap();
    let q = SpeechPhoneSequence::new(old, BoundedSequence::try_from_iter([a, b]).unwrap()).unwrap();
    let refs = (0..2)
        .map(|ordinal| {
            LanguageSpeechTokenRef::new(
                intended_basis.inventory_id().clone(),
                intended_basis.language().clone(),
                ordinal,
                intended_basis.revision_id().clone(),
                intended_basis.sequence_id().clone(),
                intended_basis.utterance_id().clone(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mapping = group(
        &refs,
        &[occurrence(0, "r1"), occurrence(1, "r1")],
        SpeechRealizationCorrespondenceKind::Realized,
    )
    .unwrap();
    let prepared = PreparedPhonemePhoneCorrespondence::prepare(&i, &p, &q, &mapping).unwrap();
    assert_eq!(prepared.phoneme_references().len(), 2);
    assert_eq!(prepared.phone_references().len(), 2);
    assert_eq!(prepared.realization().len(), 2);
    let swapped = group(
        &[occurrence(0, "r1")],
        &refs,
        SpeechRealizationCorrespondenceKind::Unresolved,
    )
    .unwrap();
    assert!(PreparedPhonemePhoneCorrespondence::prepare(&i, &p, &q, &swapped).is_err());
}
