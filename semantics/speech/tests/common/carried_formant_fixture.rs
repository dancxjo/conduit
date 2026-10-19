//! Independently authored two-word positive formant specimen: "you would".
//! Phoneme and phone IDs are supplied separately despite matching broad IPA.
#![allow(dead_code)]
#[path = "committed_carrier/commitment.rs"]
mod commitment_kernel;
#[path = "committed_carrier/constructors.rs"]
mod constructors;
use commitment_kernel::parser;
#[path = "carried_formant_fixture/phonology.rs"]
mod phonology;
use conduit_core::IeeeF32;
use conduit_language::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    correspondence::{CorrespondenceRefusal, PreparedSpeechCorrespondence},
    ipa_constructors::IpaConstructor,
    ipa_inventory::{IpaInventoryRefusal, PreparedIpaInventory},
    semantic::*,
};
pub use phonology::*;

pub fn seq<T, const N: usize>(items: impl IntoIterator<Item = T>) -> BoundedSequence<T, N> {
    BoundedSequence::try_from_iter(items).ok().unwrap()
}
pub fn evidence() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "authored you-would formant fixture".into(),
        SpeechEvidenceSource::Manual,
        Some("1".into()),
    )
    .unwrap()
}
pub fn confidence() -> SpeechConfidence {
    SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap()
}
pub fn source() -> LanguageTextRevision {
    LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("utterance".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("source/1".into()).unwrap(),
            "you would".into(),
        )
        .unwrap(),
        None,
        LinguisticDerivationProvenance::deterministic_rule("authored/you-would".into(), "1".into())
            .unwrap(),
        0,
        Some(9),
    )
    .unwrap()
}
pub fn lexical_profile() -> LanguageLexicalProfile {
    LanguageLexicalProfile::new(
        seq([
            ("you", LanguageLexicalPos::Pronoun),
            ("would", LanguageLexicalPos::Auxiliary),
        ]
        .map(|(word, pos)| {
            LanguageLexicalEntry::new(
                seq([LanguageLexicalCandidate::new(word.into(), seq([]), pos).unwrap()]),
                word.into(),
            )
            .unwrap()
        })),
        "authored/you-would".into(),
        source().material().language().clone(),
        source().provenance().clone(),
    )
    .unwrap()
}
pub fn basis(sequence: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("authored/you-would".into()).unwrap(),
        source().material().language().clone(),
        SpeechSegmentRevisionId::new("speech/1".into()).unwrap(),
        SpeechSegmentSequenceId::new(sequence.into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
pub fn occurrence(sequence: &str, ordinal: u32) -> LanguageSpeechTokenRef {
    let b = basis(sequence);
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
pub fn text_ref(kind: LanguageTextSegmentKind, start: u32, end: u32) -> LanguageTextSegmentRef {
    let source = source();
    let text = source.material();
    LanguageTextSegmentRef::new(
        kind,
        text.language().clone(),
        LanguageTextRange::new(end, start).unwrap(),
        text.revision().clone(),
        text.identity().clone(),
    )
    .unwrap()
}
pub fn word(index: usize) -> LanguageTextSegmentRef {
    let (start, end) = [(0, 3), (4, 9)][index];
    text_ref(LanguageTextSegmentKind::Word, start, end)
}
pub(super) const UNITS: [(&str, &str); 5] = [
    ("j", "j"),
    ("u", "u"),
    ("w", "w"),
    ("short-u", "ʊ"),
    ("d", "d"),
];
pub const PHONEMES: [&str; 5] = [
    "contrast/j",
    "contrast/u",
    "contrast/w",
    "contrast/short-u",
    "contrast/d",
];
pub const PHONES: [&str; 5] = [
    "target/j",
    "target/u",
    "target/w",
    "target/short-u",
    "target/d",
];
fn segment(n: usize) -> SpeechUtteranceIntentEvent {
    let r = word(usize::from(n >= 2));
    SpeechUtteranceIntentEvent::segment(
        occurrence("phones", n as u32),
        PhoneSpecification::known(PhoneId::new(PHONES[n].into()).unwrap()).unwrap(),
        PhonemeSpecification::known(PhonemeId::new(PHONEMES[n].into()).unwrap()).unwrap(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::known(10, 1).unwrap(),
            SpeechCycleSpecification::known(120, 1).unwrap(),
            SpeechIntensitySpecification::known(1, 1).unwrap(),
        )
        .unwrap(),
        evidence(),
        seq([LanguageSegmentRef::text(
            *r.kind(),
            r.language().clone(),
            r.range().clone(),
            r.revision_id().clone(),
            r.text_id().clone(),
        )
        .unwrap()]),
        StressSpecification::known(SpeechStress::Unstressed).unwrap(),
        SpeechPositionSpecification::known(match n {
            0 | 2 => SpeechWordPosition::Initial,
            1 | 4 => SpeechWordPosition::Final,
            _ => SpeechWordPosition::Medial,
        })
        .unwrap(),
    )
    .unwrap()
}
pub fn intent() -> SpeechUtteranceIntent {
    let mut events = Vec::new();
    for n in 0..5 {
        events.push(segment(n));
        if n == 1 || n == 4 {
            let r = word(usize::from(n == 4));
            events.push(
                SpeechUtteranceIntentEvent::boundary(
                    SpeechDurationSpecification::known(10, 0).unwrap(),
                    SpeechBoundarySpecification::known(if n == 1 {
                        SpeechBoundaryKind::Word
                    } else {
                        SpeechBoundaryKind::Turn
                    })
                    .unwrap(),
                    evidence(),
                    seq([LanguageSegmentRef::text(
                        *r.kind(),
                        r.language().clone(),
                        r.range().clone(),
                        r.revision_id().clone(),
                        r.text_id().clone(),
                    )
                    .unwrap()]),
                )
                .unwrap(),
            );
        }
    }
    SpeechUtteranceIntent::new(
        seq(events),
        basis("phones").inventory_id().clone(),
        source().material().language().clone(),
        evidence(),
        basis("phones").revision_id().clone(),
        basis("phones").utterance_id().clone(),
    )
    .unwrap()
}
pub fn formant_profile() -> SpeechFormantVoiceProfile {
    let inv = inventory();
    SpeechFormantVoiceProfile::new(
        "authored/you-would/formant@1".into(),
        inv.identity().clone(),
        inv.language().clone(),
        seq(inv
            .phones()
            .as_slice()
            .iter()
            .cloned()
            .zip([
                EnglishPhone::Y,
                EnglishPhone::Uw,
                EnglishPhone::W,
                EnglishPhone::Uh,
                EnglishPhone::D,
            ])
            .map(|(definition, phone)| SpeechFormantPhoneBinding::new(definition, phone).unwrap())),
    )
    .unwrap()
}
pub fn formant_boundaries() -> SpeechFormantBoundaryProfile {
    SpeechFormantBoundaryProfile::new(seq([
        SpeechFormantBoundaryBinding::new(SpeechBoundaryKind::Word, SpeechFormantBoundary::Word)
            .unwrap(),
        SpeechFormantBoundaryBinding::new(SpeechBoundaryKind::Turn, SpeechFormantBoundary::Turn)
            .unwrap(),
    ]))
    .unwrap()
}
pub fn syllables() -> Vec<SpeechSyllableSource> {
    let ipa = SpeechPhoneticTranscription::decode(&constructors::universal(
        IpaConstructor::Phonetic,
        "ju.wʊd",
        evidence(),
    ))
    .unwrap();
    [(vec![0, 1], vec![0, 1]), (vec![2, 3, 4], vec![3, 4, 5])]
        .into_iter()
        .enumerate()
        .map(|(i, (phones, indices))| {
            SpeechSyllableSource::new(
                ipa.clone(),
                seq(indices),
                SpeechPlannedSyllableIntent::new(
                    basis("phones"),
                    SpeechSyllableId::new(format!("you-would/syllable/{i}")).unwrap(),
                    Some(1),
                    seq(if i == 0 {
                        vec![
                            SpeechSyllablePosition::Onset,
                            SpeechSyllablePosition::Nucleus,
                        ]
                    } else {
                        vec![
                            SpeechSyllablePosition::Onset,
                            SpeechSyllablePosition::Nucleus,
                            SpeechSyllablePosition::Coda,
                        ]
                    }),
                    seq(phones.into_iter().map(|n| occurrence("phones", n))),
                    evidence(),
                    None,
                    StressSpecification::known(SpeechStress::Unstressed).unwrap(),
                )
                .unwrap(),
                profile().variety().clone(),
                word(i),
            )
            .unwrap()
        })
        .collect()
}
pub fn positioned_boundaries() -> Vec<SpeechPositionedBoundary> {
    [
        (2, SpeechBoundaryKind::Syllable, 0),
        (2, SpeechBoundaryKind::Word, 0),
        (5, SpeechBoundaryKind::Syllable, 1),
        (5, SpeechBoundaryKind::Word, 1),
        (5, SpeechBoundaryKind::Phrase, 1),
        (5, SpeechBoundaryKind::Turn, 1),
    ]
    .into_iter()
    .map(|(position, kind, i)| {
        SpeechPositionedBoundary::new(basis("phones"), kind, position, word(i)).unwrap()
    })
    .collect()
}
pub fn word_phonemes() -> Vec<TranslationCorrespondence> {
    [vec![0, 1], vec![2, 3, 4]]
        .into_iter()
        .enumerate()
        .map(|(i, phonemes)| {
            let w = word(i);
            TranslationCorrespondence::aligned(
                seq([LanguageSegmentRef::text(
                    *w.kind(),
                    w.language().clone(),
                    w.range().clone(),
                    w.revision_id().clone(),
                    w.text_id().clone(),
                )
                .unwrap()]),
                seq(phonemes.into_iter().map(|n| {
                    let p = occurrence("phonemes", n);
                    LanguageSegmentRef::phoneme(
                        p.inventory_id().clone(),
                        p.language().clone(),
                        *p.ordinal(),
                        p.revision_id().clone(),
                        p.sequence_id().clone(),
                        p.utterance_id().clone(),
                    )
                    .unwrap()
                })),
            )
            .unwrap()
        })
        .collect()
}

/// Owned components; caller creates the borrowing receipts in two explicit stages.
pub struct Fixture {
    pub source: LanguageTextRevision,
    pub lexical_profile: LanguageLexicalProfile,
    pub syllables: Vec<SpeechSyllableSource>,
    pub positioned_boundaries: Vec<SpeechPositionedBoundary>,
    pub word_phonemes: Vec<TranslationCorrespondence>,
    pub definitions: SpeechInventory,
    pub notation_profile: SpeechIpaNotationProfile,
    pub phone_bindings: Vec<SpeechIpaPhoneDefinitionBinding>,
    pub phoneme_bindings: Vec<SpeechIpaPhonemeDefinitionBinding>,
    pub phones: SpeechPhoneSequence,
    pub phonemes: SpeechPhonemeSequence,
    pub alignment: SpeechRealizationAlignment,
    pub intent: SpeechUtteranceIntent,
    pub formant: SpeechFormantVoiceProfile,
    pub boundaries: SpeechFormantBoundaryProfile,
}
impl Fixture {
    pub fn new() -> Self {
        Self {
            source: source(),
            lexical_profile: lexical_profile(),
            syllables: syllables(),
            positioned_boundaries: positioned_boundaries(),
            word_phonemes: word_phonemes(),
            definitions: inventory(),
            notation_profile: profile(),
            phone_bindings: phone_bindings(),
            phoneme_bindings: phoneme_bindings(),
            phones: phones(),
            phonemes: phonemes(),
            alignment: alignment(),
            intent: intent(),
            formant: formant_profile(),
            boundaries: formant_boundaries(),
        }
    }
    pub fn inventory(&self) -> Result<PreparedIpaInventory<'_>, IpaInventoryRefusal> {
        PreparedIpaInventory::prepare(
            &self.definitions,
            &self.notation_profile,
            self.notation_profile.variety(),
            self.notation_profile.revision(),
            &self.phone_bindings,
            &self.phoneme_bindings,
        )
    }
    pub fn correspondence<'a, 'i>(
        &'a self,
        inventory: &'a PreparedIpaInventory<'i>,
    ) -> Result<PreparedSpeechCorrespondence<'a, 'i>, CorrespondenceRefusal> {
        PreparedSpeechCorrespondence::prepare(
            &self.intent,
            &self.phonemes,
            &self.phones,
            self.source.material(),
            &self.alignment,
            inventory,
        )
    }
}

pub fn commit_request() -> LanguageParserBeamCommitRequest {
    let mut parser = parser::Fixture::new();
    let mut state = parser.initial(2);
    // A deterministic supplied analysis: you -> would, with one root.
    // These are actual legal Source transitions, not a forged committed state.
    for relation in ["root", "dep"] {
        let next = parser.step(&state, "right_arc", relation, "analysis/1");
        assert!(parser::accepted(&next));
        state = parser::field(&next, "state").clone();
    }
    let raw = LanguageParserNumericState::from_structured(state).unwrap();
    let state = LanguageParserState::new(
        raw.basis().clone(),
        *raw.committed(),
        *raw.depth(),
        *raw.heads(),
        raw.relation0().clone(),
        raw.relation1().clone(),
        raw.relation2().clone(),
        raw.relation3().clone(),
        *raw.stack(),
        *raw.token_count(),
        *raw.unread(),
    )
    .unwrap();
    let basis = state.basis().clone();
    let hypothesis = |active, identity| {
        LanguageParserHypothesis::new(active, identity, 0, state.clone()).unwrap()
    };
    LanguageParserBeamCommitRequest::new(
        LanguageParserAdmittedBeam::new(
            basis.clone(),
            hypothesis(true, 1),
            hypothesis(false, 2),
            hypothesis(false, 3),
            hypothesis(false, 4),
        )
        .unwrap(),
        false,
        LanguageParserRawFrontier::new(basis, 0, 2).unwrap(),
        false,
        2,
    )
    .unwrap()
}

/// Executes the accepted Language commitment Back, not a fabricated flag.
pub fn execute_commitment(
    request: &LanguageParserBeamCommitRequest,
) -> LanguageParserBeamCommitResult {
    commitment_kernel::execute(request)
}

pub fn commitment_source() -> String {
    commitment_kernel::source()
}

/// Independently owned owner receipts used to compose the complete input.
pub struct CompleteEvidence {
    pub lexical: conduit_language::lexical::PreparedLexicalTape,
    pub request: LanguageParserBeamCommitRequest,
    pub result: LanguageParserBeamCommitResult,
}
impl Fixture {
    pub fn complete_evidence(&self) -> CompleteEvidence {
        let lexical = conduit_language::lexical::prepare_lexical_tape(
            &self.source,
            &self.lexical_profile,
            None,
        )
        .unwrap();
        let request = commit_request();
        let result = execute_commitment(&request);
        CompleteEvidence {
            lexical,
            request,
            result,
        }
    }
    pub fn structure(&self) -> conduit_speech::syllable_structure::PreparedSyllableStructure<'_> {
        conduit_speech::syllable_structure::PreparedSyllableStructure::prepare(
            &self.intent,
            &self.phones,
            self.source.material(),
            &self.syllables,
            &self.positioned_boundaries,
        )
        .unwrap()
    }
}
