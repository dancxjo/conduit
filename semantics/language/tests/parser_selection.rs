#![cfg(feature = "parser-model")]
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_model_kernel.rs"]
mod parser_model_kernel;

#[test]
fn admitted_model_scores_through_ordinary_plan_play_with_exact_receipts() {
    let mut model = parser_model_kernel::ModelDriver::new();
    let indices = [
        17, 24, 348, 361, 366, 371, 373, 375, 379, 381, 383, 385, 387, 389, 391, 393, 395, 397,
        399, 401, 403, 405, 407, 409, 411,
    ];
    let first = model.score(indices);
    let second = model.score(indices);
    assert_eq!(first, second);
    assert_eq!(model.invocations(), 2);
    assert_eq!(model.receipts().len(), 2);
    assert!(model.last_receipt().is_some());
}

#[path = "common/parser_selection_driver.rs"]
mod parser_selection_driver;
#[path = "common/parser_selection_io.rs"]
mod parser_selection_io;
#[path = "common/parser_selection_values.rs"]
mod parser_selection_values;

#[path = "common/parser_selection_frontier.rs"]
mod parser_selection_frontier;
#[path = "common/parser_selection_metrics.rs"]
mod parser_selection_metrics;

#[test]
fn actual_text_enters_checked_model_action_selection_without_reference_edges() {
    use conduit_language::{lexical::prepare_lexical_tape, *};
    use parser_selection_values::*;
    use serde_json::json;
    let preparation = std::time::Instant::now();
    let mut decoder = parser_selection_driver::Decoder::new();
    let prepare_ns = preparation.elapsed().as_nanos();
    eprintln!("parser selection prepared in {prepare_ns} ns");
    let mut previous: Option<(
        LanguageTextRevision,
        conduit_language::lexical::PreparedLexicalTape,
        LanguageParserState,
    )> = None;
    let mut revisions = Vec::new();
    let mut fallback = None;
    let mut illegal_fixture = None;
    for (index, (text, stable)) in [
        ("Hello ", 5),
        ("Hello, ", 7),
        ("Hello, Travis ", 13),
        ("Hello, Travis.", 14),
    ]
    .into_iter()
    .enumerate()
    {
        eprintln!("decoding source revision {index}: {text:?}");
        let revision_start = std::time::Instant::now();
        let source = revision(
            text,
            &format!("source/greeting/{index}"),
            index as u64,
            previous.as_ref().map(|p| &p.0),
            Some(stable),
            LanguageTextFinality::Partial,
        );
        let analysis = format!("analysis/greeting/{index}");
        let (tape, rebased) = if let Some((_, prior_tape, prior_state)) = &previous {
            let rebase = PreparedParserRevisionRebase::prepare(
                prior_state,
                prior_tape,
                &source,
                LanguageAnalysisRevisionId::new(analysis.clone()).unwrap(),
                dep(),
            )
            .unwrap();
            let raw = decoder.rebase.run_native(rebase.request().clone());
            {
                let tape = prepare_lexical_tape(&source, &profile(), Some(prior_tape)).unwrap();
                assert_eq!(tape.tape(), rebase.lexical().tape());
                (tape, Some(admit_state(raw)))
            }
        } else {
            (
                prepare_lexical_tape(&source, &profile(), None).unwrap(),
                None,
            )
        };
        let count = tape
            .tape()
            .tokens()
            .iter()
            .take_while(|t| {
                matches!(t.completeness(), LanguageLexicalCompleteness::Complete)
                    && !t.candidates().is_empty()
            })
            .count();
        let lexical =
            LanguageParserAvailableLexical::new(tape.tape().clone(), count as u64).unwrap();
        let initial = rebased.unwrap_or_else(|| decoder.initial(&lexical, &analysis));
        // The checked rebase discards uncommitted edges. No reference edge is
        // carried into this analysis as an inference oracle.
        for dependent in *initial.committed() as usize..count {
            assert_eq!(initial.heads()[dependent], 5);
        }
        if index == 0 {
            illegal_fixture = Some(decoder.illegal_scored_fixture(&initial));
        }
        let invocation_before = decoder.model.invocations();
        let result = decoder.decode(&lexical, initial.clone(), true);
        let observed = parser_selection_frontier::observe(
            &result.beam,
            &lexical,
            &mut decoder.agreement,
            &mut decoder.advance,
            &mut decoder.commit,
            index == 3,
        );
        revisions.push(json!({"text":text,"source_revision":source.material().revision().get(),"analysis_revision":analysis,"source_finality":"partial","available_tokens":count,"complete":result.complete,"steps":result.steps,"decode_elapsed_ns":result.elapsed_ns,"revision_elapsed_ns":revision_start.elapsed().as_nanos(),"initial_heads":initial.heads(),"initial_unread":initial.unread(),"model_invocations":result.invocations,"invocation_first":invocation_before+1,"invocation_last":decoder.model.invocations(),"observation":observed}));
        assert!(result.invocations > 0);
        assert_eq!(
            result
                .beam
                .candidate0()
                .parser()
                .state()
                .basis()
                .source_revision(),
            source.material().revision()
        );
        if index == 3 {
            let baseline = decoder.decode(&lexical, initial, false);
            fallback = Some(
                json!({"mode":"deterministic class order","model_invocations":baseline.invocations,"complete":baseline.complete,"heads":baseline.beam.candidate0().parser().state().heads(),"score":baseline.beam.candidate0().parser().score()}),
            );
            assert_eq!(baseline.invocations, 0);
        }
        previous = Some((
            source,
            tape,
            result.beam.candidate0().parser().state().clone(),
        ));
    }
    eprintln!("decoding held-out examples");
    let heldout = parser_selection_metrics::heldout(&mut decoder);
    let memory = std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let artifact: serde_json::Value =
        serde_json::from_str(include_str!("../training/ewt_joint_v2/manifest.json")).unwrap();
    let report = json!({"profile":"finite English checked action selection","model_content_identity":artifact["model_content_identity"],"model_artifact_sha256":artifact["artifact_sha256"],"accepted_gold_state_action_ranking":artifact["heldout_incremental_gold_state_action_ranking"],"prepare_ns":prepare_ns,"revisions":revisions,"fallback":fallback,"illegal_scored_fixture":illegal_fixture,"heldout":heldout,"model_invocations":decoder.model.invocations(),"committed_model_receipts":decoder.model.receipts().len(),"rejected_checked_proposals":decoder.illegal,"refused_selected_expansions":decoder.refused_expansions,"process_memory":memory,"limitations":["four tokens; finite admitted dictionary","held-out source reconstructed from corpus forms","debug hosted Plan/Play timings include generic transport","no speech or prosody inference","explicit commit observation is not applied back into decoder beam"]});
    if let Ok(path) = std::env::var("CONDUIT_PARSER_SELECTION_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
    assert_eq!(
        decoder.model.invocations() as usize,
        decoder.model.receipts().len()
    );
    assert!(
        report["heldout"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| !row["error"]
                .as_str()
                .is_some_and(|error| error.starts_with("decoding panic:"))),
        "runtime failures remain in the report and fail this contract test"
    );
}
