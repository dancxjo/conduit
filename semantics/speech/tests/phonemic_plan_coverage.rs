#![cfg(feature = "semantic-bindings")]
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    phonemic_pronunciation::*, phonemic_pronunciation_intent::*, plan_coverage::*, semantic::*,
};
#[path = "common/vocative_language.rs"]
#[allow(dead_code)]
mod language;
#[test]
fn full_original_phonemic_words_use_existing_source_composition_and_complete_order() {
    // Supplied exact graph, not model accuracy or commitment authority.
    let case = language::case(0, "phonemic-coverage", None);
    let unit = |ipa: &str, stress| {
        SpeechPhonemicPronunciationPhoneme::new(
            PhonemeId::new(format!("phoneme/{ipa}")).unwrap(),
            stress,
        )
        .unwrap()
    };
    let row = |lemma, pos, units: &[(&str, SpeechStress)]| {
        SpeechPhonemicPronunciationRow::new(
            language::candidate(lemma, pos),
            BoundedSequence::try_from_iter(units.iter().map(|(ipa, stress)| unit(ipa, *stress)))
                .unwrap(),
        )
        .unwrap()
    };
    let profile = SpeechPhonemicPronunciationProfile::new(
        "reviewed/greeting/phonemic-v1".into(),
        case.inventory.language().clone(),
        language::speech_provenance(),
        BoundedSequence::try_from_iter([
            row(
                "hello",
                LanguageLexicalPos::Interjection,
                &[
                    ("h", SpeechStress::Unstressed),
                    ("ə", SpeechStress::Unstressed),
                    ("l", SpeechStress::Primary),
                    ("oʊ", SpeechStress::Primary),
                ],
            ),
            row(
                "Travis",
                LanguageLexicalPos::ProperNoun,
                &[
                    ("t", SpeechStress::Primary),
                    ("ɹ", SpeechStress::Primary),
                    ("æ", SpeechStress::Primary),
                    ("v", SpeechStress::Unstressed),
                    ("ɪ", SpeechStress::Unstressed),
                    ("s", SpeechStress::Unstressed),
                ],
            ),
        ])
        .unwrap(),
    )
    .unwrap();
    let prepared = case
        .selections
        .iter()
        .map(|selection| prepare_phonemic_pronunciation(selection, &profile).unwrap())
        .collect::<Vec<_>>();
    let inventory = SpeechInventoryId::new("reviewed/greeting/common-ipa-v1".into()).unwrap();
    let revision = SpeechSegmentRevisionId::new("phonemic/0".into()).unwrap();
    let utterance = SpeechUtteranceId::new("phonemic/greeting".into()).unwrap();
    let origin = |sequence: String, ordinal| {
        LanguageSpeechTokenRef::new(
            inventory.clone(),
            profile.language().clone(),
            ordinal,
            revision.clone(),
            SpeechSegmentSequenceId::new(sequence).unwrap(),
            utterance.clone(),
        )
        .unwrap()
    };
    let prosody = SpeechSegmentProsodyIntent::new(
        SpeechDurationSpecification::known(5, 1).unwrap(),
        SpeechCycleSpecification::known(200, 1).unwrap(),
        SpeechIntensitySpecification::known(1, 1).unwrap(),
    )
    .unwrap();
    let words = prepared
        .iter()
        .enumerate()
        .map(|(i, p)| {
            prepare_phonemic_pronunciation_intent(
                p,
                &origin(format!("word/{i}"), 0),
                &prosody,
                profile.provenance(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let refs = words
        .iter()
        .map(PreparedSpeechPronunciationWord::Phoneme)
        .collect::<Vec<_>>();
    let order =
        prepare_complete_spoken_order(&case.lexical, &case.participation, &case.spoken_ordinals)
            .unwrap();
    assert!(matches!(
        prepare_complete_pronunciation_layout(&order, &refs[..1]),
        Err(SpeechPlanCoverageRefusal::MissingWord)
    ));
    let swapped = [refs[1], refs[0]];
    assert!(matches!(
        prepare_complete_pronunciation_layout(&order, &swapped),
        Err(SpeechPlanCoverageRefusal::ForeignWord { word: 0 })
    ));
    let layout = prepare_complete_pronunciation_layout(&order, &refs).unwrap();
    let mut events = Vec::new();
    let mut witnesses = Vec::new();
    for (position, word) in words.iter().enumerate() {
        for event in word.intent().events().iter() {
            let SpeechUtteranceIntentEvent::Segment(original) = event else {
                panic!("phonemic segment")
            };
            let occurrence = origin("common/target-phones".into(), events.len() as u32);
            let event = SpeechUtteranceIntentEvent::segment(
                occurrence.clone(),
                original.phone().clone(),
                original.phoneme().clone(),
                original.prosody().clone(),
                original.provenance().clone(),
                original.sources().clone(),
                original.stress().clone(),
                original.word_position().clone(),
            )
            .unwrap();
            let SpeechUtteranceIntentEvent::Segment(current) = &event else {
                unreachable!()
            };
            witnesses.push(
                SpeechPhoneCompositionWitness::new(
                    admit_planned_segment_material(current).unwrap(),
                    events.len() as u32,
                    layout.clone(),
                    admit_planned_segment_material(original).unwrap(),
                    SpeechOccurrenceMembership::new(
                        inventory.clone(),
                        profile.language().clone(),
                        occurrence,
                        revision.clone(),
                        utterance.clone(),
                    )
                    .unwrap(),
                    position as u64,
                )
                .unwrap(),
            );
            events.push(event);
        }
    }
    let composite = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        inventory,
        profile.language().clone(),
        profile.provenance().clone(),
        revision,
        utterance,
    )
    .unwrap();
    let coverage =
        prepare_pronunciation_plan_coverage(&order, &refs, &composite, &witnesses).unwrap();
    assert_eq!(coverage.phone_events().len(), 10);
    assert!(core::ptr::eq(coverage.intent(), &composite));
    assert_eq!(coverage.layout(), &layout);
    for (actual, original) in coverage.words().iter().zip(&words) {
        let PreparedSpeechPronunciationWord::Phoneme(word) = actual else {
            panic!("phonemic owner")
        };
        assert!(core::ptr::eq(*word, original));
    }
    assert!(matches!(
        prepare_pronunciation_plan_coverage(&order, &refs, &composite, &witnesses[..9]),
        Err(SpeechPlanCoverageRefusal::PhoneCount)
    ));
}
