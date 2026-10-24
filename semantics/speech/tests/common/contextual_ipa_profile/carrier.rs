//! Explicit original tapes and word/syllable relations, not generated pronunciations.
use super::declaration::*;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{ipa_constructors::IpaConstructor, semantic::*};
// The shared helper also exports a scoped constructor unused by this fixture.
#[allow(dead_code)]
#[path = "../committed_carrier/constructors.rs"]
mod constructors;
pub fn intent() -> SpeechUtteranceIntent {
    SpeechUtteranceIntent::new(
        seq((0..12).map(|n| {
            let w = word(n / 4);
            SpeechUtteranceIntentEvent::segment(
                occurrence("phones", n as u32),
                PhoneSpecification::known(pid(phone_name(n))).unwrap(),
                PhonemeSpecification::known(phoneme_id(n)).unwrap(),
                SpeechSegmentProsodyIntent::new(
                    SpeechDurationSpecification::known(30, 1).unwrap(),
                    SpeechCycleSpecification::known(120, 1).unwrap(),
                    SpeechIntensitySpecification::known(1, 1).unwrap(),
                )
                .unwrap(),
                evidence(),
                seq([LanguageSegmentRef::text(
                    *w.kind(),
                    w.language().clone(),
                    w.range().clone(),
                    w.revision_id().clone(),
                    w.text_id().clone(),
                )
                .unwrap()]),
                stress(n),
                SpeechPositionSpecification::known(match n % 4 {
                    0 => SpeechWordPosition::Initial,
                    3 => SpeechWordPosition::Final,
                    _ => SpeechWordPosition::Medial,
                })
                .unwrap(),
            )
            .unwrap()
        })),
        basis("phones").inventory_id().clone(),
        language(),
        evidence(),
        basis("phones").revision_id().clone(),
        basis("phones").utterance_id().clone(),
    )
    .unwrap()
}
pub fn phones() -> SpeechPhoneSequence {
    SpeechPhoneSequence::new(
        basis("phones"),
        seq((0..12).map(|n| {
            SpeechPhoneToken::new(
                seq([]),
                confidence(),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhoneSpecification::known(pid(phone_name(n))).unwrap(),
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
        seq((0..12).map(|n| {
            SpeechPhonemeToken::new(
                confidence(),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhonemeSpecification::known(phoneme_id(n)).unwrap(),
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
    let p = profile();
    SpeechRealizationAlignment::new(
        seq((0..12).map(|n| {
            SpeechRealizationCorrespondence::aligned(
                seq([occurrence("phonemes", n)]),
                seq([occurrence("phones", n)]),
            )
            .unwrap()
        })),
        SpeechIpaInventoryNotationBasis::new(
            basis("phones").inventory_id().clone(),
            language(),
            p.clone(),
            p.revision().clone(),
            p.variety().clone(),
        )
        .unwrap(),
        basis("phones"),
        basis("phonemes"),
        evidence(),
        text_ref(LanguageTextSegmentKind::Utterance, 0, 14),
    )
    .unwrap()
}
pub fn words() -> Vec<TranslationCorrespondence> {
    (0..3)
        .map(|i| {
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
                seq((i * 4..i * 4 + 4).map(|n| {
                    let p = occurrence("phonemes", n as u32);
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
pub fn syllables() -> Vec<SpeechSyllableSource> {
    (0..6)
        .map(|i| {
            let ipa = SpeechPhoneticTranscription::decode(&constructors::universal(
                IpaConstructor::Phonetic,
                if i.is_multiple_of(2) { "tʰɑ" } else { "tɑ" },
                evidence(),
            ))
            .unwrap();
            let indices = ipa
                .units()
                .as_slice()
                .iter()
                .enumerate()
                .filter_map(|(n, u)| {
                    matches!(u.value(), SpeechPhoneticEvent::Phone(_)).then_some(n as u64)
                })
                .collect::<Vec<_>>();
            SpeechSyllableSource::new(
                ipa,
                seq(indices),
                SpeechPlannedSyllableIntent::new(
                    basis("phones"),
                    SpeechSyllableId::new(format!("g1/syllable/{i}")).unwrap(),
                    Some(1),
                    seq([
                        SpeechSyllablePosition::Onset,
                        SpeechSyllablePosition::Nucleus,
                    ]),
                    seq([occurrence("phones", i * 2), occurrence("phones", i * 2 + 1)]),
                    evidence(),
                    None,
                    stress(i as usize * 2),
                )
                .unwrap(),
                profile().variety().clone(),
                word(i as usize / 2),
            )
            .unwrap()
        })
        .collect()
}
