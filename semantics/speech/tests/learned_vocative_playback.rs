#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
//! Opt-in actual native parser receipt continuation. Teaching cases are not heldout.
#[path = "common/asr_graph_sources.rs"]
mod asr_sources;
#[path = "common/learned_playback_epoch.rs"]
mod epoch;
#[path = "common/learned_playback_evidence.rs"]
mod evidence;
#[path = "common/playback_graph.rs"]
mod graph;
#[path = "common/vocative_intent.rs"]
mod intent;
#[path = "common/vocative_language.rs"]
#[allow(dead_code)]
mod language;
#[path = "common/vocative_lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[path = "common/learned_graph_receipt.rs"]
mod receipt;
use conduit_speech::{
    intent_realization::*, lexical_pronunciation::*, pitch_trajectory::*, playback_basis::*,
};
#[test]
#[ignore = "requires actual learned native receipts via CONDUIT_LEARNED_GRAPH_RECEIPTS"]
fn actual_learned_native_graphs_feed_three_position_playback() {
    let path =
        std::env::var("CONDUIT_LEARNED_GRAPH_RECEIPTS").expect("actual receipt path required");
    assert!(std::fs::metadata(&path).unwrap().len() <= 8 * 1024 * 1024);
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.len() <= 8 * 1024 * 1024);
    let rows: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3, "exact reviewed three-position run");
    let mut seen = [false; 3];
    for row in rows {
        let admitted = receipt::admit(row).unwrap();
        let position = admitted.vocative;
        assert!(position < 3 && !seen[position]);
        seen[position] = true;
        let mut case = language::admitted_graph(
            admitted.lexical,
            admitted.basis.analysis_revision().clone(),
            admitted.arcs,
            position,
        );
        case.phones = language::parser_phone_profile();
        for (ordinal, (selection, choice)) in
            case.selections.iter().zip(&admitted.choices).enumerate()
        {
            assert_eq!(
                selection.candidate(),
                &case.lexical.tape().tokens()[ordinal].candidates()[*choice]
            );
        }
        let pronunciations = case
            .selections
            .iter()
            .map(|selection| prepare_pronunciation(selection, &case.phones).unwrap())
            .collect::<Vec<_>>();
        let composite = intent::compose(&case, &pronunciations);
        assert_eq!(composite.words.len(), case.selections.len());
        assert_eq!(composite.correspondence.len(), composite.segments.len());
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
        let epoch = epoch::commitment(&tape);
        let pcm = lifecycle::scheduler_pressure(&tape);
        assert!(!pcm.is_empty());
        evidence::retain(
            position,
            row,
            &case,
            &pronunciations,
            &composite,
            &tape,
            &pcm,
            epoch,
        );
    }
    assert!(seen.into_iter().all(|v| v));
}

#[test]
fn receipt_admission_refuses_foreign_source_choice_and_arc_coverage() {
    use conduit_language::*;
    use conduit_plot::rust_binding::NativeRustBinding;
    let case = language::case(0, "receipt/supplied", None);
    let source = case.lexical.tape().source();
    let basis = LanguageParserBasis::new(
        case.analysis.clone(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let predicted = case.arcs.iter().enumerate().map(|(dependent, arc)| {
        let head = match arc.governor() {
            LanguageDependencyHead::Root => 4,
            LanguageDependencyHead::Token(reference) => case.lexical.tape().tokens().iter().position(|t| t.identity() == reference.token()).unwrap(),
        };
        serde_json::json!({"dependent": dependent, "head": head, "canonical_arc_bytes": arc.clone().encode().unwrap()})
    }).collect::<Vec<_>>();
    let row = serde_json::json!({"text":source.material().text(),"text_identity":basis.text().get(),"source_revision":basis.source_revision().get(),"analysis_revision":basis.analysis_revision().get(),"source_material_bytes":source.clone().encode().unwrap(),"lexical_tape_bytes":case.lexical.tape().clone().encode().unwrap(),"basis":basis.encode().unwrap(),"choices":[0,0,0,0],"predicted":predicted,"complete":true});
    assert!(receipt::admit(&row).is_ok());
    let mut foreign = row.clone();
    foreign["source_revision"] = "foreign".into();
    assert!(receipt::admit(&foreign).is_err());
    let mut foreign = row.clone();
    foreign["choices"][0] = 4.into();
    assert!(receipt::admit(&foreign).is_err());
    let mut foreign = row.clone();
    foreign["predicted"][1] = foreign["predicted"][0].clone();
    assert!(receipt::admit(&foreign).is_err());
    let mut foreign = row;
    foreign["complete"] = false.into();
    assert!(receipt::admit(&foreign).is_err());
}

#[test]
fn checked_asr_events_prepare_exact_revision_and_lexical_chain() {
    let case = language::case(0, "asr/supplied", None);
    let evidence = asr_sources::prepare("initial", case.lexical.tape());
    assert_eq!(
        evidence["language_revision_bytes"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(evidence["asr_envelope_bytes"].as_array().unwrap().len(), 3);
    assert_eq!(evidence["provider_accuracy"], false);
    use conduit_language::LanguageLexicalTape;
    use conduit_plot::rust_binding::NativeRustBinding;
    let native_bytes = evidence["lexical_tape_bytes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    let native = LanguageLexicalTape::decode(&native_bytes).unwrap();
    assert!(asr_sources::admit_history(&evidence, &native).is_ok());
    let mut foreign = evidence.clone();
    foreign["asr_envelope_bytes"][1] = foreign["asr_envelope_bytes"][0].clone();
    assert!(asr_sources::admit_history(&foreign, &native).is_err());
    let mut foreign = evidence;
    foreign["language_revision_bytes"][1] = foreign["language_revision_bytes"][0].clone();
    assert!(asr_sources::admit_history(&foreign, &native).is_err());
}
#[test]
#[ignore = "exports new ASR-origin tapes for a separate actual parser run"]
fn export_asr_origin_tapes_for_actual_parser() {
    let input = std::env::var("CONDUIT_LEARNED_GRAPH_RECEIPTS").unwrap();
    let output = std::env::var("CONDUIT_ASR_GRAPH_SOURCES").unwrap();
    assert!(std::fs::metadata(&input).unwrap().len() <= 8 * 1024 * 1024);
    let rows: serde_json::Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let exports = rows
        .iter()
        .map(|row| {
            let graph = receipt::admit(row).unwrap();
            asr_sources::prepare(row["id"].as_str().unwrap(), graph.lexical.tape())
        })
        .collect::<Vec<_>>();
    if let Some(parent) = std::path::Path::new(&output).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(output, serde_json::to_vec_pretty(&exports).unwrap()).unwrap();
}
