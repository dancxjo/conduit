//! Complete Source admission before either realization, with whole retained witnesses.
use super::{intent::Composite, language::Case};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    lexical_pronunciation::PreparedPronunciation, plan_coverage::*, pronunciation_intent::*,
    semantic::*,
};

pub fn words<'a, 'basis>(
    pronunciation: &'a [PreparedPronunciation<'basis>],
    composite: &Composite,
) -> Vec<PreparedPronunciationIntent<'a, 'basis>> {
    assert_eq!(pronunciation.len(), composite.words.len());
    assert!(pronunciation.len() <= 4);
    pronunciation
        .iter()
        .zip(&composite.words)
        .map(|(pronunciation, original)| {
            let SpeechUtteranceIntentEvent::Segment(first) = &original.events()[0] else {
                panic!("checked word segment")
            };
            let word = prepare_pronunciation_intent(
                pronunciation,
                first.occurrence(),
                first.prosody(),
                first.provenance(),
            )
            .unwrap();
            assert_eq!(word.intent(), original);
            word
        })
        .collect()
}

pub fn witnesses<'a, 'word, 'basis>(
    order: &PreparedSpeechSpokenOrder<'a>,
    words: &[&'a PreparedPronunciationIntent<'word, 'basis>],
    composite: &Composite,
) -> Vec<SpeechPhoneCompositionWitness> {
    let layout = prepare_complete_phone_layout(order, words).unwrap();
    let mut witnesses = Vec::with_capacity(32);
    for (position, word) in words.iter().enumerate() {
        for original in word.intent().events().iter() {
            let SpeechUtteranceIntentEvent::Segment(original) = original else {
                panic!("checked word segment")
            };
            let original = admit_planned_segment_material(original).unwrap();
            assert!(witnesses.len() < 32);
            let current = &composite.segments[witnesses.len()];
            let target = SpeechOccurrenceMembership::new(
                composite.source.inventory_id().clone(),
                composite.source.language().clone(),
                current.occurrence().clone(),
                composite.source.revision_id().clone(),
                composite.source.utterance_id().clone(),
            )
            .unwrap();
            // Source owns offset, complete segment equality and occurrence membership.
            witnesses.push(
                SpeechPhoneCompositionWitness::new(
                    current.clone(),
                    witnesses.len() as u32,
                    layout.clone(),
                    original.clone(),
                    target,
                    position as u64,
                )
                .unwrap(),
            );
        }
    }
    witnesses
}

pub fn material(
    coverage: &PreparedSpeechPlanCoverage<'_, '_, '_>,
    case: &Case,
) -> serde_json::Value {
    fn native<T: NativeRustBinding + Clone>(value: &T) -> Vec<u8> {
        value.clone().encode().unwrap()
    }
    assert_eq!(coverage.order().lexical().tape(), case.lexical.tape());
    serde_json::json!({
        "profile":"speech-complete-spoken-coverage4@1",
        "parser_commit_authority":false,
        "complete_spoken_coverage_admitted":true,
        "source_definitions":[include_str!("../../spoken_order.conduit"),include_str!("../../phone_layout.conduit"),include_str!("../../phone_composition.conduit")],
        "complete_lexical_tape_bytes":native(case.lexical.tape()),
        "spoken_order_bytes":native(coverage.order().native()),
        "phone_layout_bytes":native(coverage.layout()),
        "phone_composition_witness_bytes":coverage.witnesses().iter().map(native).collect::<Vec<_>>(),
        "complete_utterance_intent_bytes":native(coverage.intent()),
        "original_word_intent_bytes":coverage.words().iter().map(|word| native(word.intent())).collect::<Vec<_>>(),
        "phone_event_indices":coverage.phone_events(),
    })
}
