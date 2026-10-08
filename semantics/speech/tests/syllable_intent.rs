#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{semantic::*, syllable_intent::*};
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
fn phones() -> SpeechPhoneSequence {
    let tokens = (0..3).map(|_| {
        SpeechPhoneToken::new(
            BoundedSequence::new(),
            SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
            SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
            PhoneSpecification::unknown(),
            provenance(),
            None,
        )
        .unwrap()
    });
    SpeechPhoneSequence::new(basis("r1"), BoundedSequence::try_from_iter(tokens).unwrap()).unwrap()
}
fn syllable(
    refs: &[LanguageSpeechTokenRef],
    positions: &[SpeechSyllablePosition],
    nucleus: Option<u64>,
    stress: StressSpecification,
) -> Result<SpeechPlannedSyllableIntent, conduit_plot::rust_binding::NativeBindingRefusal> {
    SpeechPlannedSyllableIntent::new(
        basis("r1"),
        SpeechSyllableId::new("syllable-1".into()).unwrap(),
        nucleus,
        BoundedSequence::try_from_iter(positions.iter().cloned()).unwrap(),
        BoundedSequence::try_from_iter(refs.iter().cloned()).unwrap(),
        provenance(),
        None,
        stress,
    )
}
fn positions() -> [SpeechSyllablePosition; 3] {
    [
        SpeechSyllablePosition::Onset,
        SpeechSyllablePosition::Nucleus,
        SpeechSyllablePosition::Coda,
    ]
}
#[test]
fn exact_original_intent_phone_material_and_all_stress_states_survive() {
    let original = intent("r1");
    let phones = phones();
    let refs = [
        occurrence(0, "r1"),
        occurrence(1, "r1"),
        occurrence(2, "r1"),
    ];
    let states = [
        StressSpecification::known(SpeechStress::Primary).unwrap(),
        StressSpecification::unknown(),
        StressSpecification::unspecified(),
        StressSpecification::not_applicable(),
        StressSpecification::variable(
            BoundedSequence::try_from_iter([SpeechStress::Primary, SpeechStress::Secondary])
                .unwrap(),
        )
        .unwrap(),
        StressSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            SpeechStress::Reduced,
        )
        .unwrap(),
    ];
    for state in states {
        let s = syllable(&refs, &positions(), Some(1), state.clone()).unwrap();
        let bytes = s.clone().encode().unwrap();
        assert_eq!(SpeechPlannedSyllableIntent::decode(&bytes).unwrap(), s);
        let p = PreparedSyllableIntent::prepare(&original, &phones, &s).unwrap();
        assert!(core::ptr::eq(p.intent(), &original));
        assert!(core::ptr::eq(p.phones(), &phones));
        assert!(core::ptr::eq(p.syllable(), &s));
        assert_eq!(p.syllable().stress(), &state);
        assert_eq!(p.syllable().nucleus_index(), &Some(1));
        assert!(p.syllable().span().is_none());
        assert_eq!(p.members().len(), 3);
        assert_eq!(p.references().len(), 3);
        assert_eq!(p.order().len(), 2);
        assert_eq!(p.sequence_basis().phones(), &phones);
    }
}
#[test]
fn source_laws_refuse_bad_position_coverage_and_nucleus() {
    let refs = [
        occurrence(0, "r1"),
        occurrence(1, "r1"),
        occurrence(2, "r1"),
    ];
    assert!(syllable(
        &refs,
        &positions()[..2],
        None,
        StressSpecification::unknown()
    )
    .is_err());
    assert!(syllable(&refs, &positions(), Some(0), StressSpecification::unknown()).is_err());
    assert!(syllable(&refs, &positions(), Some(3), StressSpecification::unknown()).is_err());
    assert!(syllable(&[], &[], None, StressSpecification::unknown()).is_err());
    let s = syllable(&refs, &positions(), None, StressSpecification::unknown()).unwrap();
    assert!(s.nucleus_index().is_none());
}
#[test]
fn full_basis_missing_members_duplicates_and_reverse_order_refuse() {
    let original = intent("r1");
    let phones = phones();
    for refs in [
        vec![
            occurrence(0, "r1"),
            occurrence(1, "foreign"),
            occurrence(2, "r1"),
        ],
        vec![
            occurrence(0, "r1"),
            occurrence(1, "r1"),
            occurrence(3, "r1"),
        ],
        vec![
            occurrence(0, "r1"),
            occurrence(1, "r1"),
            occurrence(1, "r1"),
        ],
        vec![
            occurrence(2, "r1"),
            occurrence(1, "r1"),
            occurrence(0, "r1"),
        ],
    ] {
        let s = syllable(&refs, &positions(), Some(1), StressSpecification::unknown()).unwrap();
        assert!(PreparedSyllableIntent::prepare(&original, &phones, &s).is_err());
    }
    let refs = [
        occurrence(0, "r1"),
        occurrence(1, "r1"),
        occurrence(2, "r1"),
    ];
    let s = syllable(&refs, &positions(), None, StressSpecification::unknown()).unwrap();
    assert!(PreparedSyllableIntent::prepare(&intent("foreign"), &phones, &s).is_err());
    let foreign = SpeechPhoneSequence::new(basis("foreign"), phones.tokens().clone()).unwrap();
    assert!(PreparedSyllableIntent::prepare(&original, &foreign, &s).is_err());
}

#[test]
fn supplied_seconds_span_is_checked_without_inventing_a_clock() {
    use conduit_core::IeeeF64;
    let original = intent("r1");
    let phones = phones();
    let refs = [
        occurrence(0, "r1"),
        occurrence(1, "r1"),
        occurrence(2, "r1"),
    ];
    let base = syllable(&refs, &positions(), None, StressSpecification::unknown()).unwrap();
    for (start, end, accepted) in [(-0.5, 0.5, true), (0.0, 0.0, true), (1.0, 0.0, false)] {
        let span = SpeechSegmentSpan::seconds(IeeeF64::from_value(end), IeeeF64::from_value(start))
            .unwrap();
        let s = SpeechPlannedSyllableIntent::new(
            base.basis().clone(),
            base.identity().clone(),
            *base.nucleus_index(),
            base.phone_positions().clone(),
            base.phones().clone(),
            base.provenance().clone(),
            Some(span.clone()),
            base.stress().clone(),
        )
        .unwrap();
        let prepared = PreparedSyllableIntent::prepare(&original, &phones, &s);
        assert_eq!(prepared.is_ok(), accepted);
        if let Ok(prepared) = prepared {
            assert_eq!(prepared.syllable().span(), &Some(span));
        }
    }
}
