#![cfg(feature = "parser-model")]
//! Frozen, previously observed candidate evaluation; no accepted model or current authority.
#[path = "common/causal_candidate_loader.rs"]
mod causal_candidate_loader;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_model_kernel.rs"]
mod parser_model_kernel;
#[path = "common/parser_selection_driver.rs"]
mod parser_selection_driver;
#[path = "common/parser_selection_io.rs"]
mod parser_selection_io;
#[path = "common/parser_selection_values.rs"]
mod parser_selection_values;
use causal_candidate_loader::{number, text, Inputs};
use conduit_language::{
    lexical::{prepare_lexical_tape, PreparedLexicalTape},
    *,
};
use parser_selection_values::{admit_state, profile, revision_with_text};
use serde_json::{json, Value};
use std::io::Write;
fn slots(beam: &LanguageParserScorerBeam) -> [&LanguageParserScorerHypothesis; 4] {
    [
        beam.candidate0(),
        beam.candidate1(),
        beam.candidate2(),
        beam.candidate3(),
    ]
}
fn snapshot(beam: &LanguageParserScorerBeam) -> Value {
    json!(slots(beam).iter().map(|h|json!({"active":h.parser().active(),"identity":h.parser().identity(),"score":h.parser().score(),"choices":h.choices(),"selected":h.selected(),"heads":h.parser().state().heads(),"relations":relations(h.parser().state()),"relation_subtypes":[h.parser().state().relation0().subtype().get(),h.parser().state().relation1().subtype().get(),h.parser().state().relation2().subtype().get(),h.parser().state().relation3().subtype().get()],"committed":h.parser().state().committed()})).collect::<Vec<_>>())
}
fn apply_actual_commit(
    beam: &LanguageParserScorerBeam,
    result: &LanguageParserBeamCommitResult,
) -> LanguageParserScorerBeam {
    let raws = [
        result.candidate0(),
        result.candidate1(),
        result.candidate2(),
        result.candidate3(),
    ];
    let mut updated = Vec::new();
    for (prior, raw) in slots(beam).into_iter().zip(raws) {
        assert_eq!(prior.parser().identity(), raw.identity());
        assert_eq!(prior.parser().score(), raw.score());
        assert_eq!(prior.parser().active(), raw.active());
        assert_eq!(prior.parser().state().heads(), raw.state().heads());
        updated.push(
            LanguageParserScorerHypothesis::new(
                *prior.choices(),
                LanguageParserHypothesis::new(
                    *raw.active(),
                    *raw.identity(),
                    *raw.score(),
                    admit_state(raw.state().clone()),
                )
                .unwrap(),
                *prior.selected(),
            )
            .unwrap(),
        );
    }
    LanguageParserScorerBeam::new(
        updated[0].clone(),
        updated[1].clone(),
        updated[2].clone(),
        updated[3].clone(),
    )
    .unwrap()
}
fn observe_commit(
    decoder: &mut parser_selection_driver::Decoder,
    beam: &mut LanguageParserScorerBeam,
    lexical: &LanguageParserAvailableLexical,
    pending_through: Option<u64>,
) -> Value {
    let hypotheses = slots(beam);
    let admitted = LanguageParserAdmittedBeam::new(
        hypotheses[0].parser().state().basis().clone(),
        hypotheses[0].parser().clone(),
        hypotheses[1].parser().clone(),
        hypotheses[2].parser().clone(),
        hypotheses[3].parser().clone(),
    )
    .unwrap();
    let initial = *hypotheses[0].parser().state().committed();
    let mut frontier =
        LanguageParserRawFrontier::new(admitted.basis().clone(), initial, initial).unwrap();
    let mut agreements = Vec::new();
    let mut halted = false;
    for ordinal in 0..*lexical.token_count() {
        let agreement: LanguageParserAgreement = decoder.agreement.run_native(
            LanguageParserAdmittedAgreementQuery::new(admitted.clone(), ordinal).unwrap(),
        );
        if !halted && ordinal == *frontier.stable() {
            let next: LanguageParserFrontierResult = decoder.advance.run_native(
                LanguageParserAdvanceRequest::new(
                    agreement.clone(),
                    false,
                    frontier.clone(),
                    false,
                )
                .unwrap(),
            );
            halted = !*next.accepted();
            frontier = next.frontier().clone();
        }
        agreements.push(json!({"ordinal":ordinal,"agrees":agreement.agrees(),"survivors":agreement.survivors(),"head":agreement.reference().head(),"relation":format!("{:?}",agreement.reference().relation().base()).to_ascii_lowercase()}));
    }
    let before = snapshot(beam);
    let Some(through) =
        pending_through.filter(|through| *through <= *frontier.stable() && *through > initial)
    else {
        return json!({"agreements":agreements,"stable":frontier.stable(),"committed":initial,"accepted":false,"refusal":"not requested: pending consumer dependency not yet wholly stable, or already committed","pending_consumer_through":pending_through,"before":before,"after":snapshot(beam),"actual_result_applied_to_current_beam":false});
    };
    let request =
        LanguageParserBeamCommitRequest::new(admitted, false, frontier.clone(), false, through)
            .unwrap();
    let result: LanguageParserBeamCommitResult = decoder.commit.run_native(request);
    *beam = apply_actual_commit(beam, &result);
    json!({"agreements":agreements,"stable":frontier.stable(),"committed":result.frontier().committed(),"accepted":result.accepted(),"refusal":format!("{:?}",result.refusal()),"pending_consumer_through":through,"before":before,"after":snapshot(beam),"actual_result_applied_to_current_beam":true})
}
fn relations(state: &LanguageParserState) -> [String; 4] {
    [
        state.relation0(),
        state.relation1(),
        state.relation2(),
        state.relation3(),
    ]
    .map(|r| format!("{:?}", r.base()).to_ascii_lowercase())
}

fn canonical<T: conduit_plot::rust_binding::NativeRustBinding + Clone>(value: &T) -> String {
    value
        .clone()
        .encode()
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn occupancy(beam: &LanguageParserScorerBeam) -> Value {
    let active = slots(beam)
        .into_iter()
        .filter(|h| *h.parser().active())
        .collect::<Vec<_>>();
    let graphs = active
        .iter()
        .map(|h| canonical(h.parser().state()))
        .collect::<std::collections::BTreeSet<_>>();
    let choices = active
        .iter()
        .map(|h| format!("{:?}/{:?}", h.choices(), h.selected()))
        .collect::<std::collections::BTreeSet<_>>();
    json!({"active":active.len(),"distinct_full_states":graphs.len(),"distinct_lexical_choices":choices.len(),"all_four":snapshot(beam),"beam_canonical_hex":canonical(beam)})
}
fn evaluate(
    decoder: &mut parser_selection_driver::Decoder,
    row: &Value,
    log: &mut std::fs::File,
    output: &std::path::Path,
    stem: &str,
) -> Value {
    let id = text(&row["id"]);
    let steps = row["steps"].as_array().unwrap();
    let mut prior: Option<(
        LanguageTextRevision,
        PreparedLexicalTape,
        LanguageParserScorerBeam,
    )> = None;
    let mut observations = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let trace = output.join(format!("{stem}-prefix-{index:02}-source.jsonl"));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&trace)
            .unwrap();
        std::env::set_var("CONDUIT_PARSER_SOURCE_TRACE", &trace);
        let source = revision_with_text(
            text(&step["text"]),
            &format!("cf/{id}"),
            &format!("cf/{id}/s/{index}"),
            index as u64,
            prior.as_ref().map(|p| &p.0),
            Some(number(&step["stable_prefix_scalars"]) as u32),
            LanguageTextFinality::Partial,
        );
        let analysis = format!("cf/{id}/a/{index}");
        let (tape, seeds, coalesced) = if let Some((_, old_tape, old_beam)) = &prior {
            let (t, b, c) = decoder.rebase_fresh_revision(
                old_beam,
                old_tape,
                &source,
                &LanguageAnalysisRevisionId::new(analysis.clone()).unwrap(),
            );
            (t, Some(b), c)
        } else {
            (
                prepare_lexical_tape(&source, &profile(), None).unwrap(),
                None,
                0,
            )
        };
        let count = number(&step["expected_available_count"]);
        assert_eq!(
            tape.tape().tokens().len() as u64,
            count,
            "actual lexical segmentation"
        );
        let lexical = LanguageParserAvailableLexical::new(tape.tape().clone(), count).unwrap();
        let seed = seeds.as_ref().map(occupancy);
        let result = if let Some(beam) = seeds {
            decoder.decode_from_beam(&lexical, beam, true)
        } else {
            let initial = decoder.initial(&lexical, &analysis);
            decoder.decode(&lexical, initial, true)
        };
        let mut current = result.beam;
        let commit = observe_commit(
            decoder,
            &mut current,
            &lexical,
            Some(number(&row["pending_consumer_through"])),
        );
        let trace_end = std::fs::metadata(&trace).unwrap().len();
        assert!(
            trace_end > 0 && trace_end <= 67108864,
            "actual bounded prefix Source observations"
        );
        let observation = json!({"source_trace":{"path":trace,"start_inclusive":0,"end_exclusive":trace_end},"case":id,"step":index,"text":step["text"],"source_finality":"partial","prefinal_observation":index+1<steps.len(),"source_canonical_hex":canonical(&source),"lexical_canonical_hex":canonical(tape.tape()),"available_count":count,"rebased_seed":seed,"coalesced":coalesced,"complete":result.complete,"invocations":result.invocations,"elapsed_ns":result.elapsed_ns,"commit":commit,"occupancy":occupancy(&current)});
        writeln!(log, "{}", observation).unwrap();
        log.flush().unwrap();
        observations.push(observation);
        prior = Some((source, tape, current));
    }
    // Gold labels are read only after all model and Source operations for this case.
    let beam = &prior.as_ref().unwrap().2;
    let winner = beam.candidate0().parser().state();
    let predicted = relations(winner);
    let mut uas = 0;
    let mut las = 0;
    let mut tp = 0;
    let mut fp = 0;
    let mut missed = 0;
    for (i, gold) in row["gold_heads"].as_array().unwrap().iter().enumerate() {
        let head = winner.heads()[i] == number(gold);
        let goldrel = text(&row["gold_relations"][i]).split(':').next().unwrap();
        let correct = head && predicted[i] == goldrel;
        uas += u64::from(head);
        las += u64::from(correct);
        let pv = predicted[i] == "vocative";
        let gv = goldrel == "vocative";
        tp += u64::from(pv && gv && head);
        fp += u64::from(pv && !(gv && head));
        missed += u64::from(gv && !(pv && head));
    }
    let first_commit = observations
        .iter()
        .position(|v| number(&v["commit"]["committed"]) > 0);
    let mut stability = Vec::new();
    for ordinal in 0..row["gold_heads"].as_array().unwrap().len() {
        let mut previous = None;
        let mut revisions = 0;
        let mut committed_changes = 0;
        for o in &observations {
            if number(&o["available_count"]) <= ordinal as u64 {
                continue;
            }
            let slot = &o["occupancy"]["all_four"][0];
            let edge = (
                slot["heads"][ordinal].clone(),
                slot["relations"][ordinal].clone(),
            );
            if let Some((old, committed)) = &previous {
                if old != &edge {
                    revisions += 1;
                    if *committed {
                        committed_changes += 1;
                    }
                }
            }
            previous = Some((edge, number(&slot["committed"]) > ordinal as u64));
        }
        stability.push(json!({"ordinal":ordinal,"winner_edge_revisions":revisions,"changes_after_commit":committed_changes}));
    }
    json!({"id":id,"classification":"PREVIOUSLY_OBSERVED_NONBLIND","uas_correct":uas,"las_correct":las,"tokens":row["gold_heads"].as_array().unwrap().len(),"vocative_tp":tp,"vocative_fp":fp,"vocative_fn":missed,"first_commit_step":first_commit,"nonfinal_commit":first_commit.is_some_and(|i|i+1<steps.len()),"stability":stability,"observations":observations})
}
#[test]
#[ignore = "requires reviewed external fit/cases manifest and fresh private evidence paths"]
fn external_matched_candidates_follow_same_frozen_source_histories() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let inputs=Inputs::load();
        std::fs::write(inputs.output.join("admission.json"),serde_json::to_vec_pretty(&json!({"status":"ADMITTED_NOT_YET_EVALUATED","classification":"UNACCEPTED_NONBLIND","manifest":inputs.manifest,"source_pins":inputs.pins,"case_count":inputs.cases.len()})).unwrap()).unwrap();
        let mut arms=Vec::new();
        let mut completed=0;
        for arm in &inputs.arms {
            let mut batches=Vec::new();
            for (ordinal,row) in inputs.cases.iter().enumerate() {
                let stem=format!("{}-{ordinal:02}",arm.name);
                let trace=inputs.output.join(format!("{stem}-preparation-source.jsonl"));
                std::fs::OpenOptions::new().write(true).create_new(true).open(&trace).unwrap();
                // This test drives exactly one decoder at a time. No other test
                // using the process-global diagnostic variable may run concurrently.
                std::env::set_var("CONDUIT_PARSER_SOURCE_TRACE",&trace);
                let mut log=std::fs::OpenOptions::new().write(true).create_new(true).open(inputs.output.join(format!("{stem}-prefixes.jsonl"))).unwrap();
                let mut decoder=parser_selection_driver::Decoder::with_model(arm.model());
                let preparation_end=std::fs::metadata(&trace).unwrap().len();
                assert!(preparation_end>0 && preparation_end<=67108864);
                let case=evaluate(&mut decoder,row,&mut log,&inputs.output,&stem);
                let receipts=decoder.model.receipts().iter().map(|r|json!({"model_content":r.model_content,"input":r.input,"output":r.output,"invocation":r.invocation,"work_units":r.work_units})).collect::<Vec<_>>();
                let trace_end=std::fs::metadata(&trace).unwrap().len();
                assert!(trace_end>0 && trace_end<=67108864,"actual bounded Source observations");
                let report=json!({"status":"CASE_COMPLETED","case_ordinal":ordinal,"preparation_source_trace":{"path":trace,"start_inclusive":0,"end_exclusive":trace_end,"scope":"Decoder construction ranking parity Source checks"},"prefix_source_traces":case["observations"].as_array().unwrap().iter().map(|o|o["source_trace"].clone()).collect::<Vec<_>>(),"arm":arm.name,"identity":arm.identity,"numerical_plan":decoder.model.plan,"numerical_receipts":receipts,"model_invocations":decoder.model.invocations(),"illegal":decoder.illegal,"refused_expansions":decoder.refused_expansions,"case":case});
                std::fs::write(inputs.output.join(format!("{stem}-report.json")),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
                completed+=1;
                batches.push(report);
            }
            arms.push(json!({"arm":arm.name,"batches":batches}));
        }
        assert_eq!(completed,34,"no partial cohort aggregate");
        let report=json!({"classification":"UNACCEPTED_FROZEN_NONBLIND_EVALUATION_ONLY","completed_batches":completed,"batching_scope":"sequential preparation per case; counters reset only by fresh prepared decoder; no installed capacity claim","accepted_default":false,"installed_current_controller":false,"source_evidence":"actual canonical Source trace plus compiled Source/helper pins; specialized Source Plan IDs are not exported by existing helper","manifest":inputs.manifest,"source_pins":inputs.pins,"arms":arms});
        std::fs::write(inputs.output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }).unwrap().join().unwrap();
}
