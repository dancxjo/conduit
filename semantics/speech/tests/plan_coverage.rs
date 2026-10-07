#![cfg(feature = "semantic-bindings")]
#[path = "common/vocative_intent.rs"]
mod intent;
#[path = "common/vocative_language.rs"]
mod language;
use conduit_speech::{
    lexical_pronunciation::*, plan_coverage::*, pronunciation_intent::*, semantic::*,
};

#[test]
fn full_tape_coverage_refuses_omitted_hello_and_retains_source_phone_mapping() {
    // Supplied graph fixture, not learned parser accuracy or commitment evidence.
    let case = language::case(0, "coverage/revision", None);
    assert_eq!(case.lexical.tape().tokens().len(), 2);
    assert_eq!(case.spoken_ordinals, [0, 1]);
    assert!(matches!(
        prepare_complete_spoken_order(&case.lexical, &case.participation[..1], &[0]),
        Err(SpeechPlanCoverageRefusal::MissingRole)
    ));
    assert!(matches!(
        prepare_complete_spoken_order(&case.lexical, &case.participation, &[0]),
        Err(SpeechPlanCoverageRefusal::NativeOrder(_))
    ));
    assert!(matches!(
        prepare_complete_spoken_order(&case.lexical, &case.participation, &[1, 0]),
        Err(SpeechPlanCoverageRefusal::NativeOrder(_))
    ));
    let foreign = language::case(0, "coverage/foreign-revision", None);
    assert!(matches!(
        prepare_complete_spoken_order(&case.lexical, &foreign.participation, &[0, 1]),
        Err(SpeechPlanCoverageRefusal::ForeignRole { .. })
    ));
    let order =
        prepare_complete_spoken_order(&case.lexical, &case.participation, &case.spoken_ordinals)
            .unwrap();
    let pronunciations: Vec<_> = case
        .selections
        .iter()
        .map(|selection| prepare_pronunciation(selection, &case.phones).unwrap())
        .collect();
    let composed = intent::compose(&case, &pronunciations);
    let words: Vec<_> = pronunciations
        .iter()
        .zip(&composed.words)
        .map(|(pronunciation, word)| {
            let SpeechUtteranceIntentEvent::Segment(first) = &word.events()[0] else {
                panic!("word segment")
            };
            let prepared = prepare_pronunciation_intent(
                pronunciation,
                first.occurrence(),
                first.prosody(),
                first.provenance(),
            )
            .unwrap();
            assert_eq!(prepared.intent(), word);
            prepared
        })
        .collect();
    let word_refs: Vec<_> = words.iter().collect();
    assert!(matches!(
        prepare_complete_phone_layout(&order, &word_refs[..1]),
        Err(SpeechPlanCoverageRefusal::MissingWord)
    ));
    let layout = prepare_complete_phone_layout(&order, &word_refs).unwrap();
    let mut witnesses = Vec::new();
    let mut global = 0;
    for (position, word) in words.iter().enumerate() {
        for original in word.intent().events().iter() {
            let SpeechUtteranceIntentEvent::Segment(original) = original else {
                panic!("word segment")
            };
            let original = admit_planned_segment_material(original).unwrap();
            let current = &composed.segments[global];
            let target = SpeechOccurrenceMembership::new(
                composed.source.inventory_id().clone(),
                composed.source.language().clone(),
                current.occurrence().clone(),
                composed.source.revision_id().clone(),
                composed.source.utterance_id().clone(),
            )
            .unwrap();
            assert!(SpeechPhoneCompositionWitness::new(
                current.clone(),
                ((global + 1) % 32) as u32,
                layout.clone(),
                original.clone(),
                target.clone(),
                position as u64,
            )
            .is_err());
            witnesses.push(
                SpeechPhoneCompositionWitness::new(
                    current.clone(),
                    global as u32,
                    layout.clone(),
                    original.clone(),
                    target,
                    position as u64,
                )
                .unwrap(),
            );
            global += 1;
        }
    }
    let coverage =
        prepare_speech_plan_coverage(&order, &word_refs, &composed.source, &witnesses).unwrap();
    assert_eq!(coverage.phone_events().len(), composed.segments.len());
    assert_eq!(coverage.words().len(), 2);
    assert_eq!(coverage.witnesses(), witnesses);
    assert!(matches!(
        prepare_speech_plan_coverage(&order, &word_refs, &composed.words[0], &witnesses,),
        Err(SpeechPlanCoverageRefusal::PhoneCount)
    ));
    assert!(matches!(
        prepare_speech_plan_coverage(
            &order,
            &word_refs,
            &composed.source,
            &witnesses[..witnesses.len() - 1],
        ),
        Err(SpeechPlanCoverageRefusal::PhoneCount)
    ));
}
