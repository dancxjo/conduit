use conduit_core::IeeeF32;
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
pub struct ComponentsFixture {
    pub phone_sequence: SpeechPhoneSequence,
    pub phoneme_sequence: SpeechPhonemeSequence,
    pub correspondences: Vec<SpeechPhonemePhoneCorrespondence>,
    pub syllables: Vec<SpeechPlannedSyllableIntent>,
    pub context: SpeechUtteranceIntentContext,
    pub positions: Vec<SpeechSyllablePositionSpecification>,
}
pub fn components(
    composite: &SpeechUtteranceIntent,
    text: &LanguageText,
    provenance: &SpeechEvidenceProvenance,
    variety: &LanguageVariety,
) -> ComponentsFixture {
    let features = || SpeechFeatureBundle::new(BoundedSequence::new()).unwrap();
    let confidence = || SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap();
    let q_basis = SpeechTokenSequenceBasis::new(
        composite.inventory_id().clone(),
        composite.language().clone(),
        composite.revision_id().clone(),
        SpeechSegmentSequenceId::new("common/target-phones".into()).unwrap(),
        composite.utterance_id().clone(),
    )
    .unwrap();
    let p_basis = SpeechTokenSequenceBasis::new(
        composite.inventory_id().clone(),
        composite.language().clone(),
        composite.revision_id().clone(),
        SpeechSegmentSequenceId::new("common/intended-phonemes".into()).unwrap(),
        composite.utterance_id().clone(),
    )
    .unwrap();
    let reference = |basis: &SpeechTokenSequenceBasis, ordinal: u32| {
        LanguageSpeechTokenRef::new(
            basis.inventory_id().clone(),
            basis.language().clone(),
            ordinal,
            basis.revision_id().clone(),
            basis.sequence_id().clone(),
            basis.utterance_id().clone(),
        )
        .unwrap()
    };
    let segments = composite
        .events()
        .iter()
        .filter_map(|event| {
            if let SpeechUtteranceIntentEvent::Segment(s) = event {
                Some(s)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let q_tokens = segments
        .iter()
        .map(|s| {
            SpeechPhoneToken::new(
                BoundedSequence::new(),
                confidence(),
                features(),
                s.phone().clone(),
                provenance.clone(),
                None,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let p_tokens = segments
        .iter()
        .zip(&q_tokens)
        .map(|(s, q)| {
            SpeechPhonemeToken::new(
                confidence(),
                features(),
                s.phoneme().clone(),
                provenance.clone(),
                BoundedSequence::try_from_iter([q.clone()]).unwrap(),
                None,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let phone_sequence = SpeechPhoneSequence::new(
        q_basis.clone(),
        BoundedSequence::try_from_iter(q_tokens).unwrap(),
    )
    .unwrap();
    let phoneme_sequence = SpeechPhonemeSequence::new(
        p_basis.clone(),
        BoundedSequence::try_from_iter(p_tokens).unwrap(),
    )
    .unwrap();
    let correspondences = (0..10)
        .map(|i| {
            SpeechPhonemePhoneCorrespondence::new(
                SpeechRealizationCorrespondenceKind::Realized,
                BoundedSequence::try_from_iter([reference(&p_basis, i)]).unwrap(),
                BoundedSequence::try_from_iter([reference(&q_basis, i)]).unwrap(),
                provenance.clone(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    use SpeechSyllablePosition::{Coda, Nucleus, Onset};
    // Explicit reviewed grouping: hə.ˈloʊ ˈtɹæ.vɪs; no syllabification heuristic.
    let syllable_data = [
        (
            0u32,
            2u32,
            1u32,
            SpeechStress::Unstressed,
            vec![Onset, Nucleus],
        ),
        (2, 4, 3, SpeechStress::Primary, vec![Onset, Nucleus]),
        (4, 7, 6, SpeechStress::Primary, vec![Onset, Onset, Nucleus]),
        (
            7,
            10,
            8,
            SpeechStress::Unstressed,
            vec![Onset, Nucleus, Coda],
        ),
    ];
    let syllables = syllable_data
        .iter()
        .enumerate()
        .map(|(i, (start, end, nucleus, stress, positions))| {
            SpeechPlannedSyllableIntent::new(
                q_basis.clone(),
                SpeechSyllableId::new(format!("greeting/syllable/{i}")).unwrap(),
                Some(u64::from(*nucleus - *start)),
                BoundedSequence::try_from_iter(positions.iter().copied()).unwrap(),
                BoundedSequence::try_from_iter((*start..*end).map(|i| reference(&q_basis, i)))
                    .unwrap(),
                provenance.clone(),
                None,
                StressSpecification::known(*stress).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let text_ref = LanguageTextSegmentRef::new(
        LanguageTextSegmentKind::Utterance,
        text.language().clone(),
        LanguageTextRange::new(u32::try_from(text.text().chars().count()).unwrap(), 0).unwrap(),
        text.revision().clone(),
        text.identity().clone(),
    )
    .unwrap();
    let context = SpeechUtteranceIntentContext::new(
        Some(text_ref),
        provenance.clone(),
        SpeechReferenceProjectionCapability::new("retained_metadata_only".into()).unwrap(),
        SpeechSpeakerReferenceSpecification::Unknown,
        SpeechStyleReferenceSpecification::Unspecified,
        variety.clone(),
    )
    .unwrap();
    let positions = syllable_data
        .iter()
        .flat_map(|(_, _, _, _, positions)| positions.iter().copied())
        .map(|p| SpeechSyllablePositionSpecification::known(p).unwrap())
        .collect::<Vec<_>>();
    ComponentsFixture {
        phone_sequence,
        phoneme_sequence,
        correspondences,
        syllables,
        context,
        positions,
    }
}
