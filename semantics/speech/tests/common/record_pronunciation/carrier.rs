//! Finite canonical pair fixture. Reviewed realization is explicit; no inferred
//! syllable structure, measured duration or learned-parser accuracy is claimed.
use super::inventory;
#[path = "../carried_formant_fixture.rs"]
mod base;
pub(super) use base::commitment_kernel;
use base::seq;
use conduit_core::IeeeF32;
use conduit_language::*;
use conduit_speech::{record_pronunciation::SuppliedRecordPronunciation, semantic::*};
fn evidence() -> SpeechEvidenceProvenance {
    inventory::provenance()
}
fn features() -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(seq([])).unwrap()
}

pub fn build(p: &SuppliedRecordPronunciation<'_, '_>) -> base::Fixture {
    let selection = p.selection();
    let source = selection.request().source().clone();
    let noun = p.stressed_syllable() == 0;
    let prefix = if noun { vec!["ax"] } else { vec!["w", "i"] };
    let prefix_count = prefix.len();
    let mut phone_names = prefix;
    phone_names.extend(
        p.phonetic()
            .transcription()
            .units()
            .iter()
            .filter(|u| matches!(u.value(), SpeechPhoneticEvent::Phone(_)))
            .map(|u| match u.source_spelling().get().as_str() {
                "ɹ" => "r",
                "ɛ" => "eh",
                "ɪ" => "ih",
                "k" => "k",
                "ɔ" => "ao",
                "d" => "d",
                other => panic!("unsupported reviewed record phone {other}"),
            }),
    );
    let phoneme_names: Vec<_> = phone_names
        .iter()
        .map(|name| match *name {
            "eh" => "e",
            "ao" => "ao-long",
            other => other,
        })
        .collect();
    let owned = inventory::reviewed();
    let basis = |sequence: &str| {
        SpeechTokenSequenceBasis::new(
            owned.inventory.identity().clone(),
            source.material().language().clone(),
            SpeechSegmentRevisionId::new("record-pair/segments@1".into()).unwrap(),
            SpeechSegmentSequenceId::new(sequence.into()).unwrap(),
            SpeechUtteranceId::new(format!("record-pair/{}", source.material().text())).unwrap(),
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
    let word = |i: usize| {
        let range = selection.request().source().material().text();
        let start = if i == 0 { 0 } else { prefix_count as u32 + 1 };
        let end = if i == 0 {
            prefix_count as u32
        } else {
            range.chars().count() as u32
        };
        LanguageTextSegmentRef::new(
            LanguageTextSegmentKind::Word,
            source.material().language().clone(),
            LanguageTextRange::new(end, start).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap()
    };
    let word_ref = |i: usize| {
        let w = word(i);
        LanguageSegmentRef::text(
            *w.kind(),
            w.language().clone(),
            w.range().clone(),
            w.revision_id().clone(),
            w.text_id().clone(),
        )
        .unwrap()
    };
    let stress = |n: usize| {
        if n == prefix_count + if noun { 1 } else { 3 } {
            SpeechStress::Primary
        } else {
            SpeechStress::Unstressed
        }
    };
    let phones = SpeechPhoneSequence::new(
        basis("phones"),
        seq(phone_names.iter().map(|name| {
            SpeechPhoneToken::new(
                seq([]),
                SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
                features(),
                PhoneSpecification::known(PhoneId::new(format!("uk-record/phone/{name}")).unwrap())
                    .unwrap(),
                evidence(),
                None,
            )
            .unwrap()
        })),
    )
    .unwrap();
    let phonemes = SpeechPhonemeSequence::new(
        basis("phonemes"),
        seq(phoneme_names.iter().map(|name| {
            SpeechPhonemeToken::new(
                SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
                features(),
                PhonemeSpecification::known(PhonemeId::new(format!("uk-record/{name}")).unwrap())
                    .unwrap(),
                evidence(),
                seq([]),
                None,
            )
            .unwrap()
        })),
    )
    .unwrap();
    let mut events = Vec::new();
    for n in 0..phone_names.len() {
        let i = usize::from(n >= prefix_count);
        events.push(
            SpeechUtteranceIntentEvent::segment(
                occurrence("phones", n),
                phones.tokens()[n].phone().clone(),
                phonemes.tokens()[n].phoneme().clone(),
                SpeechSegmentProsodyIntent::new(
                    SpeechDurationSpecification::known(
                        *if n == prefix_count + 3 {
                            p.long_duration()
                        } else {
                            p.short_duration()
                        }
                        .denominator(),
                        *if n == prefix_count + 3 {
                            p.long_duration()
                        } else {
                            p.short_duration()
                        }
                        .numerator_seconds(),
                    )
                    .unwrap(),
                    SpeechCycleSpecification::known(120, 1).unwrap(),
                    SpeechIntensitySpecification::known(1, 1).unwrap(),
                )
                .unwrap(),
                evidence(),
                seq([word_ref(i)]),
                StressSpecification::known(stress(n)).unwrap(),
                SpeechPositionSpecification::known(if n == 0 || n == prefix_count {
                    SpeechWordPosition::Initial
                } else if n + 1 == prefix_count || n + 1 == phone_names.len() {
                    SpeechWordPosition::Final
                } else {
                    SpeechWordPosition::Medial
                })
                .unwrap(),
            )
            .unwrap(),
        );
        if n + 1 == prefix_count || n + 1 == phone_names.len() {
            events.push(
                SpeechUtteranceIntentEvent::boundary(
                    SpeechDurationSpecification::known(10, 0).unwrap(),
                    SpeechBoundarySpecification::known(if i == 0 {
                        SpeechBoundaryKind::Word
                    } else {
                        SpeechBoundaryKind::Turn
                    })
                    .unwrap(),
                    evidence(),
                    seq([word_ref(i)]),
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
        seq((0..phone_names.len()).map(|n| {
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
    let word_phonemes = (0..2)
        .map(|i| {
            let range = if i == 0 {
                0..prefix_count
            } else {
                prefix_count..phone_names.len()
            };
            TranslationCorrespondence::aligned(
                seq([word_ref(i)]),
                seq(range.map(|n| {
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
        "reviewed/uk-record/compact@1".into(),
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
            .map(|(phone, target)| SpeechFormantPhoneBinding::new(phone, target).unwrap())),
    )
    .unwrap();
    let ipa_body = format!(
        "{}.{}",
        if noun { "ə" } else { "wi" },
        p.phonetic().transcription().original()
    );
    let ipa = conduit_speech::ipa_phonetic::phonetic_from_ipa(ipa_body, evidence()).unwrap();
    let phone_indices: Vec<_> = ipa
        .transcription()
        .units()
        .iter()
        .enumerate()
        .filter_map(|(i, u)| matches!(u.value(), SpeechPhoneticEvent::Phone(_)).then_some(i as u64))
        .collect();
    assert_eq!(phone_indices.len(), phone_names.len());
    let split = prefix_count + if noun { 3 } else { 2 };
    let groups = [
        (0, prefix_count, if noun { 0 } else { 1 }),
        (prefix_count, split, 1),
        (split, phone_names.len(), if noun { 0 } else { 1 }),
    ];
    let syllables = groups
        .iter()
        .enumerate()
        .map(|(i, &(start, end, nucleus))| {
            let stress = if (noun && i == 1) || (!noun && i == 2) {
                SpeechStress::Primary
            } else {
                SpeechStress::Unstressed
            };
            SpeechSyllableSource::new(
                ipa.transcription().clone(),
                seq(phone_indices[start..end].iter().copied()),
                SpeechPlannedSyllableIntent::new(
                    basis("phones"),
                    SpeechSyllableId::new(format!("uk-record/syllable/{i}")).unwrap(),
                    Some(nucleus as u64),
                    seq((0..end - start).map(|j| {
                        if j < nucleus {
                            SpeechSyllablePosition::Onset
                        } else if j == nucleus {
                            SpeechSyllablePosition::Nucleus
                        } else {
                            SpeechSyllablePosition::Coda
                        }
                    })),
                    seq((start..end).map(|n| occurrence("phones", n))),
                    evidence(),
                    None,
                    StressSpecification::known(stress).unwrap(),
                )
                .unwrap(),
                owned.profile.variety().clone(),
                word(usize::from(i > 0)),
            )
            .unwrap()
        })
        .collect();
    let positioned_boundaries = [
        (prefix_count, SpeechBoundaryKind::Syllable, 0),
        (prefix_count, SpeechBoundaryKind::Word, 0),
        (split, SpeechBoundaryKind::Syllable, 1),
        (phone_names.len(), SpeechBoundaryKind::Syllable, 1),
        (phone_names.len(), SpeechBoundaryKind::Word, 1),
        (phone_names.len(), SpeechBoundaryKind::Phrase, 1),
        (phone_names.len(), SpeechBoundaryKind::Turn, 1),
    ]
    .into_iter()
    .map(|(position, kind, i)| {
        SpeechPositionedBoundary::new(basis("phones"), kind, position as u32, word(i)).unwrap()
    })
    .collect();
    base::Fixture {
        source,
        lexical_profile: selection.lexical_profile().clone(),
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
        boundaries: base::formant_boundaries(),
    }
}
