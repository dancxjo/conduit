#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
//! Recorded Source fact continuation; no provider, physical playback or joint neural claim.
#[path = "common/learned_playback_epoch.rs"]
mod epoch;
#[path = "common/learned_playback_evidence.rs"]
mod evidence;
#[path = "common/playback_graph.rs"]
mod graph;
#[path = "common/vocative_intent.rs"]
#[allow(dead_code)]
mod intent;
#[path = "common/vocative_language.rs"]
#[allow(dead_code)]
mod language;
#[path = "common/vocative_lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    intent_realization::*, lexical_pronunciation::*, pitch_trajectory::*, playback_basis::*,
};

#[test]
#[ignore = "requires retained actual Source session events and exact revision history"]
fn early_admitted_vocative_fact_prepares_only_its_word_before_final() {
    let events =
        std::fs::read_to_string(std::env::var("CONDUIT_STABLE_PARSER_EVENTS").unwrap()).unwrap();
    let snapshot = events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| row["event"] == "snapshot" && row["committed"].as_u64() == Some(1))
        .expect("actual early Source commitment snapshot");
    let bytes: Vec<u8> = serde_json::from_value(snapshot["stable_fact_bytes"][0].clone()).unwrap();
    let fact = LanguageParserJointStableFact::decode(&bytes).unwrap();
    let query = fact.query();
    assert_eq!(
        query.beam().lexical().tape().source().finality(),
        &LanguageTextFinality::Partial
    );
    let dependent = *query.dependent() as usize;
    let state = query.beam().candidate0().parser().state();
    let relation = match dependent {
        0 => state.relation0(),
        1 => state.relation1(),
        2 => state.relation2(),
        3 => state.relation3(),
        _ => panic!("finite admitted dependent"),
    };
    let head = state.heads()[dependent];
    assert!(head < 4);
    let token = |ordinal: usize| {
        LanguageAnalysisTokenRef::new(
            query.beam().basis().analysis_revision().clone(),
            query.beam().lexical().tape().tokens()[ordinal]
                .identity()
                .clone(),
        )
        .unwrap()
    };
    let governor = token(head as usize);
    let subtype = if relation.subtype().get().is_empty() {
        None
    } else {
        Some(LanguageDependencySubtype::new(relation.subtype().get().clone()).unwrap())
    };
    let arc = LanguageDependencyArc::new(
        token(dependent),
        LanguageDependencyHead::token(governor.revision().clone(), governor.token().clone())
            .unwrap(),
        LanguageDependencyRelation::new(*relation.base(), subtype).unwrap(),
    )
    .unwrap();
    let admission = LanguageParserStableDependencyAdmission::new(
        arc,
        fact.clone(),
        head,
        relation.subtype().clone(),
    )
    .unwrap();
    let commit_row = events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| {
            row["event"] == "dependency-commit"
                && row["source_revision"] == snapshot["source_revision"]
        })
        .expect("actual Source contiguous commit output");
    let runtime_bytes: Vec<u8> =
        serde_json::from_value(commit_row["native_runtime_bytes"].clone()).unwrap();
    let committed = LanguageParserCommittedDependencyAdmission::new(
        admission,
        LanguageParserJointCommitQuery::new(fact.clone()).unwrap(),
        LanguageParserJointRuntimeBeam::decode(&runtime_bytes).unwrap(),
    )
    .unwrap();
    let revisions: Vec<Vec<u8>> = serde_json::from_slice(
        &std::fs::read(std::env::var("CONDUIT_STABLE_PARSER_HISTORY").unwrap()).unwrap(),
    )
    .unwrap();
    let mut previous = None;
    for bytes in revisions {
        let revision = LanguageTextRevision::decode(&bytes).unwrap();
        previous = Some(
            prepare_lexical_tape(
                &revision,
                query.beam().lexical().tape().profile(),
                previous.as_ref(),
            )
            .unwrap(),
        );
        if revision.material().revision() == query.beam().basis().source_revision() {
            break;
        }
    }
    let stable =
        language::admitted_stable_vocative(previous.unwrap(), committed.admission().clone());
    assert_eq!(stable.admission.fact(), &fact);
    assert_eq!(stable.case.spoken_ordinals, [dependent]);
    assert_eq!(stable.case.arcs.len(), 1);
    let pronunciations = stable
        .case
        .selections
        .iter()
        .map(|selection| prepare_pronunciation(selection, &stable.case.phones).unwrap())
        .collect::<Vec<_>>();
    let composite = intent::compose(&stable.case, &pronunciations);
    assert_eq!(composite.words.len(), 1);
    let linguistic = composite.linguistic(&stable.case);
    let offers = linguistic
        .iter()
        .zip(&composite.events)
        .map(|(admitted, event)| OfferedSegmentPitch {
            event: *event,
            admission: admitted.accepted().pitch(),
        })
        .collect::<Vec<_>>();
    let pitch = prepare_utterance_pitch(&composite.source, &offers).unwrap();
    assert!(!composite.segments.is_empty());
    assert_eq!(pitch.source(), &composite.source);
    let realized = prepare_intent_realization(
        &composite.source,
        &stable.case.inventory,
        &stable.case.voice,
        &stable.case.boundaries,
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
    let tape = prepare_speech_playback_tape(&realized, &pitch, &bindings, 7).unwrap();
    let played = epoch::commitment(&tape);
    let pcm = lifecycle::scheduler_pressure(&tape);
    assert!(!pcm.is_empty());
    let graph_receipt = serde_json::json!({
        "parser_snapshot": snapshot,
        "dependency_admission_bytes": stable.admission.clone().encode().unwrap(),
        "committed_dependency_admission_bytes": committed.encode().unwrap(),
        "contiguous_commit_custody_admitted": true
    });
    evidence::retain(
        dependent,
        &graph_receipt,
        &stable.case,
        &pronunciations,
        &composite,
        &tape,
        &pcm,
        played.clone(),
        &realized,
        &pitch,
    );
    if let Ok(path) = std::env::var("CONDUIT_STABLE_SPEECH_EVIDENCE") {
        let evidence = serde_json::json!({
            "proof": "retained-partial-source-fact-speech@1",
            "parser_snapshot": snapshot,
            "dependency_admission_bytes": stable.admission.clone().encode().unwrap(),
            "spoken_ordinals": stable.case.spoken_ordinals,
            "utterance_intent_bytes": composite.source.clone().encode().unwrap(),
            "playback_basis_bytes": tape.basis().clone().encode().unwrap(),
            "pcm_frames": pcm.len()/2,
            "played_epoch": played,
            "physical_playback": false,
            "fargan_neural_waveform": false,
            "protected_later_parser_revision": false
        });
        std::fs::write(path, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
    }
}
