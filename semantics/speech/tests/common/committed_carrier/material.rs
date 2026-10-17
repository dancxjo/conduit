//! Explicit reviewed fixture data. IDs and realization groups are authored
//! independently; target-phone spelling never generates an underlying phoneme.
use conduit_core::IeeeF32;
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{ipa_constructors::IpaConstructor, semantic::*};

pub fn seq<T, const N: usize>(items: impl IntoIterator<Item = T>) -> BoundedSequence<T, N> {
    BoundedSequence::try_from_iter(items).ok().unwrap()
}
pub fn evidence() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "reviewed would-you-weigh fixture".into(),
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
            "would you weigh".into(),
        )
        .unwrap(),
        None,
        LinguisticDerivationProvenance::deterministic_rule("reviewed/fixture".into(), "1".into())
            .unwrap(),
        0,
        Some(15),
    )
    .unwrap()
}
pub fn lexical_profile() -> LanguageLexicalProfile {
    LanguageLexicalProfile::new(
        seq([
            ("would", LanguageLexicalPos::Auxiliary),
            ("you", LanguageLexicalPos::Pronoun),
            ("weigh", LanguageLexicalPos::Verb),
        ]
        .map(|(word, pos)| {
            LanguageLexicalEntry::new(
                seq([LanguageLexicalCandidate::new(word.into(), seq([]), pos).unwrap()]),
                word.into(),
            )
            .unwrap()
        })),
        "reviewed/would-you-weigh".into(),
        source().material().language().clone(),
        source().provenance().clone(),
    )
    .unwrap()
}
pub fn basis(sequence: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("reviewed/utterance".into()).unwrap(),
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
    let text = source();
    let text = text.material();
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
    let (start, end) = [(0, 5), (6, 9), (10, 15)][index];
    text_ref(LanguageTextSegmentKind::Word, start, end)
}
pub const PHONEMES: [&str; 7] = [
    "contrast/w",
    "contrast/short-u",
    "contrast/d",
    "contrast/j",
    "contrast/u",
    "contrast/w",
    "contrast/diphthong",
];
pub const PHONES: [&str; 7] = [
    "target/w",
    "target/short-u",
    "target/affricate",
    "target/u",
    "target/w",
    "target/e",
    "target/glide",
];
pub fn inventory() -> SpeechInventory {
    let phonemes = [
        ("contrast/w", "w"),
        ("contrast/short-u", "ʊ"),
        ("contrast/d", "d"),
        ("contrast/j", "j"),
        ("contrast/u", "u"),
        ("contrast/diphthong", "eɪ"),
    ];
    let phones = [
        ("target/w", "w"),
        ("target/short-u", "ʊ"),
        ("target/affricate", "d͡ʒ"),
        ("target/u", "u"),
        ("target/e", "e"),
        ("target/glide", "j"),
    ];
    SpeechInventory::new(
        basis("phones").inventory_id().clone(),
        source().material().language().clone(),
        seq(phonemes.map(|(id, ipa)| {
            SpeechPhoneme::new(
                seq([]),
                seq([]),
                None,
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhonemeId::new(id.into()).unwrap(),
                ipa.into(),
                seq([]),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })),
        seq(phones.map(|(id, ipa)| {
            SpeechPhone::new(
                seq([]),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhoneId::new(id.into()).unwrap(),
                ipa.into(),
                SpeechSegmentStatus::Allophonic,
            )
            .unwrap()
        })),
    )
    .unwrap()
}
pub fn profile() -> SpeechIpaNotationProfile {
    let units = [
        ("w", "w"),
        ("short-u", "ʊ"),
        ("d", "d"),
        ("j", "j"),
        ("u", "u"),
        ("e", "e"),
        ("i", "ɪ"),
        ("affricate", "d͡ʒ"),
    ];
    SpeechIpaNotationProfile::new(
        seq([]),
        SpeechIpaNotationProfileId::new("reviewed/ipa@1".into()).unwrap(),
        basis("phones").inventory_id().clone(),
        evidence(),
        SpeechSegmentRevisionId::new("inventory/3".into()).unwrap(),
        seq(units.map(|(id, ipa)| {
            SpeechIpaUnitDefinition::new(
                SpeechIpaUnitId::new(id.into()).unwrap(),
                SpeechIpaUnitKind::Segment,
                evidence(),
                SpeechIpaSpelling::new(ipa.into()).unwrap(),
            )
            .unwrap()
        })),
        LanguageVariety::new(
            VarietyId::new("reviewed/coalescence-offglide".into()).unwrap(),
            source().material().language().clone(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn phone_bindings() -> Vec<SpeechIpaPhoneDefinitionBinding> {
    ["w", "short-u", "affricate", "u", "e", "glide"]
        .into_iter()
        .zip(["w", "short-u", "affricate", "u", "e", "j"])
        .map(|(id, unit)| {
            SpeechIpaPhoneDefinitionBinding::new(
                PhoneId::new(format!("target/{id}")).unwrap(),
                evidence(),
                seq([SpeechIpaUnitId::new(unit.into()).unwrap()]),
            )
            .unwrap()
        })
        .collect()
}
pub fn phoneme_bindings() -> Vec<SpeechIpaPhonemeDefinitionBinding> {
    ["w", "short-u", "d", "j", "u", "diphthong"]
        .into_iter()
        .zip([
            vec!["w"],
            vec!["short-u"],
            vec!["d"],
            vec!["j"],
            vec!["u"],
            vec!["e", "i"],
        ])
        .map(|(id, units)| {
            SpeechIpaPhonemeDefinitionBinding::new(
                PhonemeId::new(format!("contrast/{id}")).unwrap(),
                evidence(),
                seq(units
                    .into_iter()
                    .map(|unit| SpeechIpaUnitId::new(unit.into()).unwrap())),
            )
            .unwrap()
        })
        .collect()
}
pub fn phones() -> SpeechPhoneSequence {
    SpeechPhoneSequence::new(
        basis("phones"),
        seq(PHONES.map(|id| {
            SpeechPhoneToken::new(
                seq([]),
                confidence(),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhoneSpecification::known(PhoneId::new(id.into()).unwrap()).unwrap(),
                evidence(),
                None,
            )
            .unwrap()
        })),
    )
    .unwrap()
}
pub fn phonemes() -> SpeechPhonemeSequence {
    SpeechPhonemeSequence::new(
        basis("phonemes"),
        seq(PHONEMES.map(|id| {
            SpeechPhonemeToken::new(
                confidence(),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhonemeSpecification::known(PhonemeId::new(id.into()).unwrap()).unwrap(),
                evidence(),
                seq([]),
                None,
            )
            .unwrap()
        })),
    )
    .unwrap()
}
pub fn alignment() -> SpeechRealizationAlignment {
    let profile = profile();
    let groups = [
        (vec![0], vec![0]),
        (vec![1], vec![1]),
        (vec![2, 3], vec![2]),
        (vec![4], vec![3]),
        (vec![5], vec![4]),
        (vec![6], vec![5, 6]),
    ];
    SpeechRealizationAlignment::new(
        seq(groups.map(|(left, right)| {
            SpeechRealizationCorrespondence::aligned(
                seq(left.into_iter().map(|n| occurrence("phonemes", n))),
                seq(right.into_iter().map(|n| occurrence("phones", n))),
            )
            .unwrap()
        })),
        SpeechIpaInventoryNotationBasis::new(
            basis("phones").inventory_id().clone(),
            source().material().language().clone(),
            profile.clone(),
            profile.revision().clone(),
            profile.variety().clone(),
        )
        .unwrap(),
        basis("phones"),
        basis("phonemes"),
        evidence(),
        text_ref(LanguageTextSegmentKind::Utterance, 0, 15),
    )
    .unwrap()
}
pub fn intent() -> SpeechUtteranceIntent {
    let duration = [
        SpeechDurationSpecification::known(20, 1).unwrap(),
        SpeechDurationSpecification::unknown(),
        SpeechDurationSpecification::unspecified(),
        SpeechDurationSpecification::not_applicable(),
        SpeechDurationSpecification::variable(seq([
            SpeechExactDuration::new(10, 1).unwrap(),
            SpeechExactDuration::new(20, 1).unwrap(),
        ]))
        .unwrap(),
        SpeechDurationSpecification::gradient(
            confidence(),
            SpeechExactDuration::new(20, 1).unwrap(),
        )
        .unwrap(),
    ];
    let phoneme_for_phone = [Some(0), Some(1), None, Some(4), Some(5), Some(6), Some(6)];
    let events = (0..7).map(|n| {
        let words = if n == 2 {
            vec![word(0), word(1)]
        } else {
            vec![word(if n < 2 {
                0
            } else if n < 4 {
                1
            } else {
                2
            })]
        };
        SpeechUtteranceIntentEvent::segment(
            occurrence("phones", n as u32),
            phones().tokens()[n].phone().clone(),
            phoneme_for_phone[n]
                .map(|i| phonemes().tokens()[i].phoneme().clone())
                .unwrap_or_else(PhonemeSpecification::not_applicable),
            SpeechSegmentProsodyIntent::new(
                duration[n % 6].clone(),
                SpeechCycleSpecification::unspecified(),
                SpeechIntensitySpecification::unknown(),
            )
            .unwrap(),
            evidence(),
            seq(words.into_iter().map(|r| {
                LanguageSegmentRef::text(
                    *r.kind(),
                    r.language().clone(),
                    r.range().clone(),
                    r.revision_id().clone(),
                    r.text_id().clone(),
                )
                .unwrap()
            })),
            StressSpecification::unspecified(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap()
    });
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
pub fn syllables() -> Vec<SpeechSyllableSource> {
    let ipa = SpeechPhoneticTranscription::decode(&super::constructors::universal(
        IpaConstructor::Phonetic,
        "wʊ.d͡ʒu.ˈwej",
        evidence(),
    ))
    .unwrap();
    [
        (vec![0, 1], vec![0, 1]),
        (vec![2, 3], vec![3, 4]),
        (vec![4, 5, 6], vec![7, 8, 9]),
    ]
    .into_iter()
    .enumerate()
    .map(|(word_index, (phones, indices))| {
        SpeechSyllableSource::new(
            ipa.clone(),
            seq(indices),
            SpeechPlannedSyllableIntent::new(
                basis("phones"),
                SpeechSyllableId::new(format!("syllable/{word_index}")).unwrap(),
                Some(1),
                seq(if phones.len() == 3 {
                    vec![
                        SpeechSyllablePosition::Onset,
                        SpeechSyllablePosition::Nucleus,
                        SpeechSyllablePosition::Coda,
                    ]
                } else {
                    vec![
                        SpeechSyllablePosition::Onset,
                        SpeechSyllablePosition::Nucleus,
                    ]
                }),
                seq(phones.into_iter().map(|n| occurrence("phones", n))),
                evidence(),
                None,
                StressSpecification::known(if word_index == 2 {
                    SpeechStress::Primary
                } else {
                    SpeechStress::Unstressed
                })
                .unwrap(),
            )
            .unwrap(),
            profile().variety().clone(),
            word(word_index),
        )
        .unwrap()
    })
    .collect()
}
pub fn boundaries() -> Vec<SpeechPositionedBoundary> {
    [
        (1, SpeechBoundaryKind::Phone, 0),
        (2, SpeechBoundaryKind::Syllable, 0),
        (2, SpeechBoundaryKind::Word, 0),
        (4, SpeechBoundaryKind::Morpheme, 1),
        (4, SpeechBoundaryKind::Word, 1),
        (7, SpeechBoundaryKind::Phrase, 2),
        (7, SpeechBoundaryKind::BreathGroup, 2),
        (7, SpeechBoundaryKind::Turn, 2),
    ]
    .into_iter()
    .map(|(position, kind, word_index)| {
        SpeechPositionedBoundary::new(basis("phones"), kind, position, word(word_index)).unwrap()
    })
    .collect()
}

/// Existing bounded segment correspondence retains the supplied text-to-phoneme
/// links independently of the separate phoneme-to-phone realization component.
pub fn word_phonemes() -> Vec<TranslationCorrespondence> {
    [vec![0, 1, 2], vec![3, 4], vec![5, 6]]
        .into_iter()
        .enumerate()
        .map(|(word_index, phonemes)| {
            let w = word(word_index);
            TranslationCorrespondence::aligned(
                seq([LanguageSegmentRef::text(
                    *w.kind(),
                    w.language().clone(),
                    w.range().clone(),
                    w.revision_id().clone(),
                    w.text_id().clone(),
                )
                .unwrap()]),
                seq(phonemes.into_iter().map(|ordinal| {
                    let p = occurrence("phonemes", ordinal);
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
