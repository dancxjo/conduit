#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
//! Actual committed words joined to the existing intent/terminal formant route.
//! This does not establish #5212 shared-IR conformance or a neural realization.
#[path = "../src/committed_plan_coverage.rs"]
mod committed_coverage;
#[path = "common/learned_playback_coverage.rs"]
mod coverage;
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
    committed_token_role, intent_realization::*, lexical_pronunciation::*, pitch_trajectory::*,
    plan_coverage, playback_basis::*, semantic::*,
};

fn bounded_read(name: &str) -> Vec<u8> {
    let path = std::env::var(name).unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() <= 32 * 1024 * 1024);
    std::fs::read(path).unwrap()
}
fn native<T: NativeRustBinding>(value: &serde_json::Value) -> T {
    T::decode(&serde_json::from_value::<Vec<u8>>(value.clone()).unwrap()).unwrap()
}

#[test]
#[ignore = "requires actual four-revision Source commitments and their prepared role receipt"]
fn actual_committed_complete_greeting_retains_custody_through_formant_projection() {
    let material: serde_json::Value =
        serde_json::from_slice(&bounded_read("CONDUIT_WORD_STREAM_COMMITTED_ROLES")).unwrap();
    let events = String::from_utf8(bounded_read("CONDUIT_WORD_STREAM_EVENTS")).unwrap();
    let snapshot = events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| {
            row["event"] == "snapshot" && row["source_sequence"] == 3 && row["committed"] == 3
        })
        .unwrap();
    let runtime: LanguageParserJointRuntimeBeam = native(&snapshot["beam_bytes"]);
    let beam = runtime.beam();
    assert_eq!(
        beam.lexical().tape().source().material().text(),
        "Hello, Travis."
    );
    let mut previous = None;
    for bytes in material["source_revision_history_bytes"]
        .as_array()
        .unwrap()
    {
        let revision: LanguageTextRevision = native(bytes);
        previous = Some(
            prepare_lexical_tape(
                &revision,
                beam.lexical().tape().profile(),
                previous.as_ref(),
            )
            .unwrap(),
        );
    }
    let lexical = previous.unwrap();
    assert_eq!(lexical.tape(), beam.lexical().tape());
    let state = beam.candidate0().parser().state();
    let token = |ordinal: usize| {
        LanguageAnalysisTokenRef::new(
            beam.basis().analysis_revision().clone(),
            lexical.tape().tokens()[ordinal].identity().clone(),
        )
        .unwrap()
    };
    let arcs = (0..4)
        .map(|dependent| {
            let relation = match dependent {
                0 => state.relation0(),
                1 => state.relation1(),
                2 => state.relation2(),
                _ => state.relation3(),
            };
            let head = state.heads()[dependent];
            let governor = if head == 4 {
                LanguageDependencyHead::root()
            } else {
                let reference = token(head as usize);
                LanguageDependencyHead::token(
                    reference.revision().clone(),
                    reference.token().clone(),
                )
                .unwrap()
            };
            let subtype = (!relation.subtype().get().is_empty())
                .then(|| LanguageDependencySubtype::new(relation.subtype().get().clone()).unwrap());
            LanguageDependencyArc::new(
                token(dependent),
                governor,
                LanguageDependencyRelation::new(*relation.base(), subtype).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let choices = beam
        .candidate0()
        .choices()
        .iter()
        .map(|choice| *choice as usize)
        .collect::<Vec<_>>();
    let mut case = language::admitted_graph_with_choices(
        lexical,
        beam.basis().analysis_revision().clone(),
        arcs,
        2,
        &choices,
    );
    case.phones = language::parser_phone_profile();
    assert_eq!(case.spoken_ordinals, [0, 2]);
    let commitments = material["commitments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| {
            native::<LanguageParserCommittedDependencyAdmission>(
                &word["committed_dependency_admission_bytes"],
            )
        })
        .collect::<Vec<_>>();
    let roles = commitments
        .iter()
        .map(|commitment| {
            committed_token_role::prepare_committed_token_role(&case.lexical, commitment).unwrap()
        })
        .collect::<Vec<_>>();
    assert!(roles
        .iter()
        .all(|role| matches!(role.role().result().role(), SpeechTextTokenRole::Spoken)));
    let pronunciations = case
        .selections
        .iter()
        .map(|selection| prepare_pronunciation(selection, &case.phones).unwrap())
        .collect::<Vec<_>>();
    let composite = intent::compose_with_duration(
        &case,
        &pronunciations,
        SpeechDurationSpecification::known(5, 1).unwrap(),
    );
    let order = plan_coverage::prepare_complete_spoken_order(
        &case.lexical,
        &case.participation,
        &case.spoken_ordinals,
    )
    .unwrap();
    let words = coverage::words(&pronunciations, &composite);
    let word_refs = words.iter().collect::<Vec<_>>();
    let witnesses = coverage::witnesses(&order, &word_refs, &composite);
    let complete = plan_coverage::prepare_speech_plan_coverage(
        &order,
        &word_refs,
        &composite.source,
        &witnesses,
    )
    .unwrap();
    let role_refs = roles.iter().collect::<Vec<_>>();
    let committed =
        committed_coverage::prepare_committed_speech_plan_coverage(&complete, &role_refs).unwrap();
    assert_eq!(committed.coverage().intent(), &composite.source);
    assert_eq!(committed.commitments().len(), 2);
    assert!(matches!(
        committed_coverage::prepare_committed_speech_plan_coverage(&complete, &role_refs[..1]),
        Err(committed_coverage::CommittedPlanCoverageRefusal::MissingCommitment)
    ));
    let swapped = [role_refs[1], role_refs[0]];
    assert!(matches!(
        committed_coverage::prepare_committed_speech_plan_coverage(&complete, &swapped),
        Err(committed_coverage::CommittedPlanCoverageRefusal::ForeignCommitment { word: 0 })
    ));
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
    let tape = prepare_speech_playback_tape(&realized, &pitch, &bindings, 7).unwrap();
    let pcm = lifecycle::scheduler_pressure(&tape);
    assert!(
        pcm.len() / 2 >= 2 * 8000,
        "actual Source duration, without PCM padding"
    );
    let graph = serde_json::json!({
        "schema":"speech/actual-word-stream-committed-plan@1",
        "committed_dependencies":material["commitments"],
        "final_runtime_bytes":snapshot["beam_bytes"],
        "original_linguistic_intent_bytes":composite.source.clone().encode().unwrap(),
        "commit_custody_adapter_verified":true,
        "shared_ir_5212_completion":false,
        "fargan_neural_waveform":false,
    });
    evidence::retain(
        2,
        &graph,
        &case,
        &pronunciations,
        &composite,
        &tape,
        &pcm,
        serde_json::json!({"scope":"generated PCM only; no played acknowledgment", "played_frames":0}),
        &realized,
        &pitch,
        &coverage::material(&complete, &case),
    );
}
