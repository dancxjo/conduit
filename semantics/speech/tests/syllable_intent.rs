#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_language::LanguageVariety;
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

fn text_material() -> LanguageText {
    LanguageText::new(
        LanguageTextId::new("source".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        LanguageTextRevisionId::new("source-r1".into()).unwrap(),
        "tat tat".into(),
    )
    .unwrap()
}
fn word(start: u32, end: u32, revision: &str) -> LanguageTextSegmentRef {
    LanguageTextSegmentRef::new(
        LanguageTextSegmentKind::Word,
        LanguageId::new("en".into()).unwrap(),
        LanguageTextRange::new(end, start).unwrap(),
        LanguageTextRevisionId::new(revision.into()).unwrap(),
        LanguageTextId::new("source".into()).unwrap(),
    )
    .unwrap()
}
fn grouped(first: u32, revision: &str) -> SpeechSyllableSource {
    let refs = (first..first + 3).map(|n| occurrence(n, "r1"));
    let grouping = SpeechPlannedSyllableIntent::new(
        basis("r1"),
        SpeechSyllableId::new(format!("word-{first}")).unwrap(),
        Some(1),
        BoundedSequence::try_from_iter(positions()).unwrap(),
        BoundedSequence::try_from_iter(refs).unwrap(),
        provenance(),
        None,
        StressSpecification::known(SpeechStress::Primary).unwrap(),
    )
    .unwrap();
    let ipa = conduit_speech::ipa_phonetic::phonetic_from_ipa("ˈtat".into(), provenance()).unwrap();
    SpeechSyllableSource::new(
        ipa.transcription().clone(),
        BoundedSequence::try_from_iter([1, 2, 3]).unwrap(),
        grouping,
        LanguageVariety::new(
            VarietyId::new("explicit-variety".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
        )
        .unwrap(),
        word(
            if first == 0 { 0 } else { 4 },
            if first == 0 { 3 } else { 7 },
            revision,
        ),
    )
    .unwrap()
}
#[test]
fn two_words_keep_source_ipa_stress_and_ordered_boundaries_after_readmission() {
    use conduit_speech::syllable_structure::PreparedSyllableStructure;
    let original = intent("r1");
    let three = phones();
    let phones = SpeechPhoneSequence::new(
        basis("r1"),
        BoundedSequence::try_from_iter(
            three
                .tokens()
                .as_slice()
                .iter()
                .cloned()
                .chain(three.tokens().as_slice().iter().cloned()),
        )
        .unwrap(),
    )
    .unwrap();
    let text = text_material();
    let sources: Vec<_> = [grouped(0, "source-r1"), grouped(3, "source-r1")]
        .into_iter()
        .map(|s| SpeechSyllableSource::decode(&s.encode().unwrap()).unwrap())
        .collect();
    let boundaries: Vec<_> = [
        SpeechBoundaryKind::Phone,
        SpeechBoundaryKind::Syllable,
        SpeechBoundaryKind::Word,
        SpeechBoundaryKind::Phrase,
        SpeechBoundaryKind::BreathGroup,
        SpeechBoundaryKind::Turn,
    ]
    .into_iter()
    .enumerate()
    .map(|(n, kind)| {
        SpeechPositionedBoundary::new(basis("r1"), kind, n as u32 + 1, word(4, 7, "source-r1"))
            .unwrap()
    })
    .map(|b| SpeechPositionedBoundary::decode(&b.encode().unwrap()).unwrap())
    .collect();
    let prepared =
        PreparedSyllableStructure::prepare(&original, &phones, &text, &sources, &boundaries)
            .unwrap();
    assert!(core::ptr::eq(prepared.text(), &text));
    assert_eq!(prepared.syllables().len(), 2);
    assert_eq!(prepared.ipa_receipts().len(), 6);
    assert_eq!(prepared.boundary_order().len(), 5);
    for source in prepared.sources() {
        assert_eq!(source.ipa().original(), "ˈtat");
        assert_eq!(
            source.syllable().basis().revision_id(),
            &SpeechSegmentRevisionId::new("r1".into()).unwrap()
        );
        assert_eq!(
            source.variety().identity(),
            &conduit_language::VarietyId::new("explicit-variety".into()).unwrap()
        );
        assert_eq!(
            source.syllable().stress(),
            &StressSpecification::known(SpeechStress::Primary).unwrap()
        );
    }
    let mut reversed = boundaries.clone();
    reversed.reverse();
    assert!(
        PreparedSyllableStructure::prepare(&original, &phones, &text, &sources, &reversed).is_err()
    );
    let foreign = [grouped(0, "source-old"), grouped(3, "source-r1")];
    assert!(
        PreparedSyllableStructure::prepare(&original, &phones, &text, &foreign, &boundaries)
            .is_err()
    );
    let duplicated = [sources[0].clone(), sources[0].clone()];
    assert!(PreparedSyllableStructure::prepare(
        &original,
        &phones,
        &text,
        &duplicated,
        &boundaries
    )
    .is_err());
}

#[test]
fn known_stress_without_nucleus_and_nonphone_ipa_members_refuse() {
    let source = grouped(0, "source-r1");
    let s = source.syllable();
    let no_nucleus = SpeechPlannedSyllableIntent::new(
        s.basis().clone(),
        s.identity().clone(),
        None,
        s.phone_positions().clone(),
        s.phones().clone(),
        s.provenance().clone(),
        None,
        s.stress().clone(),
    )
    .unwrap();
    assert!(SpeechSyllableSource::new(
        source.ipa().clone(),
        source.ipa_phone_indices().clone(),
        no_nucleus,
        source.variety().clone(),
        source.word().clone()
    )
    .is_err());
    // The original leading stress occurrence is an event, never a phone.
    assert!(
        SpeechSyllableIpaMember::new(0, source.ipa().units()[0].clone(), source.clone()).is_err()
    );
    assert!(
        SpeechSyllableIpaMember::new(0, source.ipa().units()[2].clone(), source.clone()).is_err()
    );
}

#[test]
fn contradictory_position_roles_and_missing_ipa_indices_refuse() {
    use conduit_speech::syllable_structure::PreparedSyllableStructure;
    let original = intent("r1");
    let phones = phones();
    let text = text_material();
    let base = grouped(0, "source-r1");
    for roles in [
        [
            SpeechSyllablePosition::Coda,
            SpeechSyllablePosition::Nucleus,
            SpeechSyllablePosition::Onset,
        ],
        [
            SpeechSyllablePosition::Nucleus,
            SpeechSyllablePosition::Nucleus,
            SpeechSyllablePosition::Coda,
        ],
    ] {
        let s = base.syllable();
        let wrong = SpeechPlannedSyllableIntent::new(
            s.basis().clone(),
            s.identity().clone(),
            Some(1),
            BoundedSequence::try_from_iter(roles).unwrap(),
            s.phones().clone(),
            s.provenance().clone(),
            None,
            s.stress().clone(),
        )
        .unwrap();
        let source = SpeechSyllableSource::new(
            base.ipa().clone(),
            base.ipa_phone_indices().clone(),
            wrong,
            base.variety().clone(),
            base.word().clone(),
        )
        .unwrap();
        assert!(
            PreparedSyllableStructure::prepare(&original, &phones, &text, &[source], &[]).is_err()
        );
    }
    for indices in [[0, 2, 3], [1, 2, 99], [3, 2, 1]] {
        let source = SpeechSyllableSource::new(
            base.ipa().clone(),
            BoundedSequence::try_from_iter(indices).unwrap(),
            base.syllable().clone(),
            base.variety().clone(),
            base.word().clone(),
        )
        .unwrap();
        assert!(
            PreparedSyllableStructure::prepare(&original, &phones, &text, &[source], &[]).is_err()
        );
    }
}
