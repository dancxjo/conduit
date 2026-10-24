//! Authored finite Ed/we/record pronunciation specimen. No dictionary/parser claim.
// Reuses the existing private parser harness also imported by the Language fixture.
#[allow(clippy::duplicate_mod)]
#[path = "../carried_formant_fixture.rs"]
mod base;
#[path = "../record_pronunciation/inventory.rs"]
pub mod inventory;
use base::seq;
use conduit_core::IeeeF32;
use conduit_language::*;
use conduit_speech::semantic::*;
fn evidence() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "authored Ed/we/record vocative specimen; not a dictionary claim".into(),
        SpeechEvidenceSource::Manual,
        Some("1".into()),
    )
    .unwrap()
}
pub fn build(
    source: &LanguageTextRevision,
    lexical_profile: &LanguageLexicalProfile,
    vocative: usize,
    rich: Option<&conduit_speech::linguistic_prosody::PreparedLinguisticProsody<'_>>,
    record: &conduit_speech::record_pronunciation::SuppliedRecordPronunciation<'_, '_>,
) -> base::Fixture {
    build_with_record_cycle(source, lexical_profile, vocative, rich, record, false)
}

/// Explicit acoustic experiment: retain the accepted verb phones, stress and
/// duration, but let only record use the existing authored stress-cycle profile.
/// The baseline build and every Ed/we/vocative control remain unchanged.
pub fn build_with_record_profile_cycle(
    source: &LanguageTextRevision,
    lexical_profile: &LanguageLexicalProfile,
    vocative: usize,
    rich: Option<&conduit_speech::linguistic_prosody::PreparedLinguisticProsody<'_>>,
    record: &conduit_speech::record_pronunciation::SuppliedRecordPronunciation<'_, '_>,
) -> base::Fixture {
    build_with_record_cycle(source, lexical_profile, vocative, rich, record, true)
}

fn build_with_record_cycle(
    source: &LanguageTextRevision,
    lexical_profile: &LanguageLexicalProfile,
    vocative: usize,
    rich: Option<&conduit_speech::linguistic_prosody::PreparedLinguisticProsody<'_>>,
    record: &conduit_speech::record_pronunciation::SuppliedRecordPronunciation<'_, '_>,
    record_profile_cycle: bool,
) -> base::Fixture {
    let words: Vec<_> = source.material().text().split(' ').collect();
    let mut spans = Vec::new();
    let mut cursor = 0;
    for word in &words {
        spans.push((cursor, cursor + word.chars().count() as u32));
        cursor += word.chars().count() as u32 + 1;
    }
    assert_eq!(record.stressed_syllable(), 1);
    let record_phones: Vec<_> = record
        .phonetic()
        .transcription()
        .units()
        .iter()
        .filter(|unit| matches!(unit.value(), SpeechPhoneticEvent::Phone(_)))
        .map(|unit| match unit.source_spelling().get().as_str() {
            "ɹ" => "r",
            "ɪ" => "ih",
            "k" => "k",
            "ɔ" => "ao",
            "d" => "d",
            other => panic!("unsupported accepted verb phone {other}"),
        })
        .collect();
    assert_eq!(record_phones, ["r", "ih", "k", "ao", "d"]);
    let mut names = Vec::new();
    let mut owners = Vec::new();
    let mut groups = Vec::new();
    for (w, word) in words.iter().enumerate() {
        let start = names.len();
        let (phones, syllables): (&[&str], &[(usize, usize, usize)]) = match *word {
            "Ed" => (&["eh", "d"], &[(0, 2, 0)]),
            "we" => (&["w", "i"], &[(0, 2, 1)]),
            "record" => (&record_phones, &[(0, 2, 1), (2, 5, 1)]),
            _ => panic!("unsupported authored word"),
        };
        names.extend(phones.iter().copied());
        owners.extend(std::iter::repeat_n(w, phones.len()));
        groups.extend(
            syllables
                .iter()
                .map(|&(a, b, n)| (start + a, start + b, n, w)),
        );
    }
    let owned = inventory::reviewed();
    let basis = |sequence: &str| {
        SpeechTokenSequenceBasis::new(
            owned.inventory.identity().clone(),
            source.material().language().clone(),
            SpeechSegmentRevisionId::new("vocative/segments@1".into()).unwrap(),
            SpeechSegmentSequenceId::new(sequence.into()).unwrap(),
            SpeechUtteranceId::new(source.material().text().clone()).unwrap(),
        )
        .unwrap()
    };
    let occurrence = |sequence: &str, n: usize| {
        let b = basis(sequence);
        LanguageSpeechTokenRef::new(
            b.inventory_id().clone(),
            b.language().clone(),
            n as u32,
            b.revision_id().clone(),
            b.sequence_id().clone(),
            b.utterance_id().clone(),
        )
        .unwrap()
    };
    let word = |w: usize| {
        LanguageTextSegmentRef::new(
            LanguageTextSegmentKind::Word,
            source.material().language().clone(),
            LanguageTextRange::new(spans[w].1, spans[w].0).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap()
    };
    let word_ref = |w: usize| {
        let r = word(w);
        LanguageSegmentRef::text(
            *r.kind(),
            r.language().clone(),
            r.range().clone(),
            r.revision_id().clone(),
            r.text_id().clone(),
        )
        .unwrap()
    };
    let phones = SpeechPhoneSequence::new(
        basis("phones"),
        seq(names.iter().map(|n| {
            SpeechPhoneToken::new(
                seq([]),
                SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhoneSpecification::known(PhoneId::new(format!("uk-record/phone/{n}")).unwrap())
                    .unwrap(),
                evidence(),
                None,
            )
            .unwrap()
        })),
    )
    .unwrap();
    let phoneme_name = |n: &str| {
        match n {
            "eh" => "e",
            "ao" => "ao-long",
            other => other,
        }
        .to_string()
    };
    let phonemes = SpeechPhonemeSequence::new(
        basis("phonemes"),
        seq(names.iter().map(|n| {
            SpeechPhonemeToken::new(
                SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhonemeSpecification::known(
                    PhonemeId::new(format!("uk-record/{}", phoneme_name(n))).unwrap(),
                )
                .unwrap(),
                evidence(),
                seq([]),
                None,
            )
            .unwrap()
        })),
    )
    .unwrap();
    let record_duration = |long: bool| {
        let duration = if long {
            record.long_duration()
        } else {
            record.short_duration()
        };
        assert_eq!(100 % *duration.denominator(), 0);
        SpeechDurationSpecification::known(
            100,
            *duration.numerator_seconds() * (100 / *duration.denominator()),
        )
        .unwrap()
    };
    let record_nuclei: Vec<_> = groups
        .iter()
        .filter(|(_, _, _, word)| words[*word] == "record")
        .map(|(start, _, nucleus, _)| start + nucleus)
        .collect();
    let stressed_nucleus = record_nuclei[record.stressed_syllable() as usize];
    assert_eq!(names[stressed_nucleus], "ao");
    let stress = |n: usize| {
        if n == stressed_nucleus {
            SpeechStress::Primary
        } else {
            SpeechStress::Unstressed
        }
    };
    let mut events = Vec::new();
    for n in 0..names.len() {
        let w = owners[n];
        let selected = rich.filter(|_| w == vocative);
        events.push(
            SpeechUtteranceIntentEvent::segment(
                occurrence("phones", n),
                phones.tokens()[n].phone().clone(),
                phonemes.tokens()[n].phoneme().clone(),
                selected
                    .map(|p| p.selected_realization().segment().clone())
                    .unwrap_or_else(|| {
                        SpeechSegmentProsodyIntent::new(
                            if words[w] == "record" {
                                record_duration(n == stressed_nucleus)
                            } else {
                                SpeechDurationSpecification::known(100, 10).unwrap()
                            },
                            if record_profile_cycle && words[w] == "record" {
                                SpeechCycleSpecification::unspecified()
                            } else {
                                SpeechCycleSpecification::known(120, 1).unwrap()
                            },
                            SpeechIntensitySpecification::known(1, 1).unwrap(),
                        )
                        .unwrap()
                    }),
                evidence(),
                seq([word_ref(w)]),
                StressSpecification::known(stress(n)).unwrap(),
                SpeechPositionSpecification::known(if n == 0 || owners[n - 1] != w {
                    SpeechWordPosition::Initial
                } else if n + 1 == names.len() || owners[n + 1] != w {
                    SpeechWordPosition::Final
                } else {
                    SpeechWordPosition::Medial
                })
                .unwrap(),
            )
            .unwrap(),
        );
        if n + 1 == names.len() || owners[n + 1] != w {
            events.push(
                SpeechUtteranceIntentEvent::boundary(
                    selected
                        .map(|p| p.boundary().duration().clone())
                        .unwrap_or_else(|| SpeechDurationSpecification::known(100, 0).unwrap()),
                    selected
                        .map(|p| p.boundary().kind().clone())
                        .unwrap_or_else(|| {
                            SpeechBoundarySpecification::known(if n + 1 == names.len() {
                                SpeechBoundaryKind::Turn
                            } else {
                                SpeechBoundaryKind::Word
                            })
                            .unwrap()
                        }),
                    evidence(),
                    seq([word_ref(w)]),
                )
                .unwrap(),
            );
        }
    }
    let intent = SpeechUtteranceIntent::new(
        seq(events),
        basis("phones").inventory_id().clone(),
        source.material().language().clone(),
        evidence(),
        basis("phones").revision_id().clone(),
        basis("phones").utterance_id().clone(),
    )
    .unwrap();
    let alignment = SpeechRealizationAlignment::new(
        seq((0..names.len()).map(|n| {
            SpeechRealizationCorrespondence::aligned(
                seq([occurrence("phonemes", n)]),
                seq([occurrence("phones", n)]),
            )
            .unwrap()
        })),
        SpeechIpaInventoryNotationBasis::new(
            owned.inventory.identity().clone(),
            source.material().language().clone(),
            owned.profile.clone(),
            owned.profile.revision().clone(),
            owned.profile.variety().clone(),
        )
        .unwrap(),
        basis("phones"),
        basis("phonemes"),
        evidence(),
        LanguageTextSegmentRef::new(
            LanguageTextSegmentKind::Utterance,
            source.material().language().clone(),
            LanguageTextRange::new(source.material().text().chars().count() as u32, 0).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let word_phonemes = (0..words.len())
        .map(|w| {
            TranslationCorrespondence::aligned(
                seq([word_ref(w)]),
                seq((0..names.len()).filter(|n| owners[*n] == w).map(|n| {
                    let r = occurrence("phonemes", n);
                    LanguageSegmentRef::phoneme(
                        r.inventory_id().clone(),
                        r.language().clone(),
                        *r.ordinal(),
                        r.revision_id().clone(),
                        r.sequence_id().clone(),
                        r.utterance_id().clone(),
                    )
                    .unwrap()
                })),
            )
            .unwrap()
        })
        .collect();
    let formant = SpeechFormantVoiceProfile::new(
        "authored/vocative/compact@1".into(),
        owned.inventory.identity().clone(),
        source.material().language().clone(),
        seq(owned
            .inventory
            .phones()
            .iter()
            .cloned()
            .zip([
                EnglishPhone::R,
                EnglishPhone::Eh,
                EnglishPhone::Ih,
                EnglishPhone::K,
                EnglishPhone::Ao,
                EnglishPhone::D,
                EnglishPhone::Ax,
                EnglishPhone::W,
                EnglishPhone::Iy,
            ])
            .map(|(p, t)| SpeechFormantPhoneBinding::new(p, t).unwrap())),
    )
    .unwrap();
    let ipa = conduit_speech::ipa_phonetic::phonetic_from_ipa(
        words
            .iter()
            .map(|w| match *w {
                "Ed" => "ɛd",
                "we" => "wi",
                "record" => record.phonetic().transcription().original().as_str(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
            .join("."),
        evidence(),
    )
    .unwrap();
    let indices: Vec<_> = ipa
        .transcription()
        .units()
        .iter()
        .enumerate()
        .filter_map(|(i, u)| matches!(u.value(), SpeechPhoneticEvent::Phone(_)).then_some(i as u64))
        .collect();
    let syllables = groups
        .iter()
        .enumerate()
        .map(|(i, &(a, b, n, w))| {
            SpeechSyllableSource::new(
                ipa.transcription().clone(),
                seq(indices[a..b].iter().copied()),
                SpeechPlannedSyllableIntent::new(
                    basis("phones"),
                    SpeechSyllableId::new(format!("vocative/syllable/{i}")).unwrap(),
                    Some(n as u64),
                    seq((0..b - a).map(|j| {
                        if j < n {
                            SpeechSyllablePosition::Onset
                        } else if j == n {
                            SpeechSyllablePosition::Nucleus
                        } else {
                            SpeechSyllablePosition::Coda
                        }
                    })),
                    seq((a..b).map(|n| occurrence("phones", n))),
                    evidence(),
                    None,
                    StressSpecification::known(stress(a + n)).unwrap(),
                )
                .unwrap(),
                owned.profile.variety().clone(),
                word(w),
            )
            .unwrap()
        })
        .collect();
    let mut positioned_boundaries = Vec::new();
    for &(_, b, _, w) in &groups {
        positioned_boundaries.push(
            SpeechPositionedBoundary::new(
                basis("phones"),
                SpeechBoundaryKind::Syllable,
                b as u32,
                word(w),
            )
            .unwrap(),
        );
    }
    for w in 0..words.len() {
        let end = owners.iter().rposition(|i| *i == w).unwrap() + 1;
        for kind in [
            SpeechBoundaryKind::Word,
            SpeechBoundaryKind::Phrase,
            SpeechBoundaryKind::Turn,
        ] {
            positioned_boundaries.push(
                SpeechPositionedBoundary::new(basis("phones"), kind, end as u32, word(w)).unwrap(),
            );
        }
    }
    positioned_boundaries
        .sort_by_key(|boundary| (*boundary.position(), *boundary.source().range().start()));
    base::Fixture {
        source: source.clone(),
        lexical_profile: lexical_profile.clone(),
        syllables,
        positioned_boundaries,
        word_phonemes,
        definitions: owned.inventory,
        notation_profile: owned.profile,
        phone_bindings: owned.phones,
        phoneme_bindings: owned.phonemes,
        phones,
        phonemes,
        alignment,
        intent,
        formant,
        boundaries: SpeechFormantBoundaryProfile::new(seq([
            SpeechFormantBoundaryBinding::new(
                SpeechBoundaryKind::Word,
                SpeechFormantBoundary::Word,
            )
            .unwrap(),
            SpeechFormantBoundaryBinding::new(
                SpeechBoundaryKind::Phrase,
                SpeechFormantBoundary::Phrase,
            )
            .unwrap(),
            SpeechFormantBoundaryBinding::new(
                SpeechBoundaryKind::Turn,
                SpeechFormantBoundary::Turn,
            )
            .unwrap(),
        ]))
        .unwrap(),
    }
}
