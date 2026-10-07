#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
#[path = "common/vocative_anchor.rs"]
mod anchor;
#[path = "common/vocative_evidence.rs"]
mod evidence;
#[path = "common/playback_graph.rs"]
mod graph;
#[path = "common/vocative_intent.rs"]
mod intent;
#[path = "common/vocative_language.rs"]
mod language;
#[path = "common/vocative_lifecycle.rs"]
mod lifecycle;
use conduit_speech::{
    intent_realization::*, lexical_pronunciation::*, pitch_trajectory::*, playback_basis::*,
    semantic::*,
};
#[test]
fn supplied_graph_full_word_pipeline_at_three_vocative_positions() {
    for position in 0..3 {
        let case = language::case(position, &format!("r/{position}"), None);
        let pronunciations = case
            .selections
            .iter()
            .map(|selection| prepare_pronunciation(selection, &case.phones).unwrap())
            .collect::<Vec<_>>();
        let composite = intent::compose(&case, &pronunciations);
        let linguistic = composite.linguistic(&case);
        let offers = linguistic
            .iter()
            .zip(&composite.events)
            .map(|(admission, event)| OfferedSegmentPitch {
                event: *event,
                admission: admission.accepted().pitch(),
            })
            .collect::<Vec<_>>();
        let pitch = prepare_utterance_pitch(&composite.source, &offers).unwrap();
        let realized = prepare_intent_realization(
            &composite.source,
            &case.inventory,
            &case.voice,
            &case.boundaries,
        )
        .unwrap();
        let bindings = linguistic
            .iter()
            .zip(&composite.events)
            .map(|(admitted, event)| PlaybackLinguisticBinding {
                event: *event,
                admitted,
            })
            .collect::<Vec<_>>();
        let overflow = (0..33)
            .map(|_| PlaybackLinguisticBinding {
                event: composite.events[0],
                admitted: &linguistic[0],
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            prepare_speech_playback_tape(&realized, &pitch, &overflow, 7),
            Err(PlaybackPreparationRefusal::OccurrenceLimit)
        ));
        let tape = prepare_speech_playback_tape(&realized, &pitch, &bindings, 7).unwrap();
        assert_eq!(case.vocative, position);
        assert!(case
            .arcs
            .iter()
            .all(|arc| arc.dependent().revision() == &case.analysis));
        assert_eq!(
            tape.basis().links().linguistic_bases().len(),
            case.selections.len()
        );
        lifecycle::native_table_refusals(&tape);
        anchor::native_anchor_refusals(&tape);
        let pcm = lifecycle::scheduler_pressure(&tape);
        assert_eq!(composite.words.len(), case.selections.len());
        assert_eq!(composite.correspondence.len(), composite.segments.len());
        let rich = tape
            .basis()
            .links()
            .occurrences()
            .iter()
            .filter(|occurrence| {
                *tape
                    .basis()
                    .links()
                    .linguistic_bases()
                    .iter()
                    .nth(*occurrence.linguistic_index() as usize)
                    .unwrap()
                    .mode()
                    == SpeechPlaybackProsodyMode::Rich
            })
            .count();
        assert_eq!(rich, 6);
        let next_case =
            language::case((position + 1) % 3, &format!("next/{position}"), Some(&case));
        assert_ne!(
            case.lexical.tape().source().material().text(),
            next_case.lexical.tape().source().material().text()
        );
        let next_pronunciations = next_case
            .selections
            .iter()
            .map(|selection| prepare_pronunciation(selection, &next_case.phones).unwrap())
            .collect::<Vec<_>>();
        let next_composite = intent::compose(&next_case, &next_pronunciations);
        let next_linguistic = next_composite.linguistic(&next_case);
        let next_offers = next_linguistic
            .iter()
            .zip(&next_composite.events)
            .map(|(admission, event)| OfferedSegmentPitch {
                event: *event,
                admission: admission.accepted().pitch(),
            })
            .collect::<Vec<_>>();
        let next_pitch = prepare_utterance_pitch(&next_composite.source, &next_offers).unwrap();
        let next_realized = prepare_intent_realization(
            &next_composite.source,
            &next_case.inventory,
            &next_case.voice,
            &next_case.boundaries,
        )
        .unwrap();
        let next_bindings = next_linguistic
            .iter()
            .zip(&next_composite.events)
            .map(|(admitted, event)| PlaybackLinguisticBinding {
                event: *event,
                admitted,
            })
            .collect::<Vec<_>>();
        let next_tape =
            prepare_speech_playback_tape(&next_realized, &next_pitch, &next_bindings, 7).unwrap();
        anchor::lineage_refusals(&tape, &next_tape);
        lifecycle::preplay_replan(&tape, &next_tape);
        lifecycle::exercise(&tape, &next_tape);
        evidence::retain(
            position,
            &case,
            &pronunciations,
            &composite,
            &tape,
            &pcm,
            &next_tape,
        );
        let plan = graph::plan(&tape);
        assert!(!plan.fragments.is_empty());
    }
}
