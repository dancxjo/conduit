#![cfg(feature = "parser-model")]
//! Explicit candidate evaluation; conditioned training metrics never enter decode.
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
use conduit_language::{
    lexical::{prepare_lexical_tape, PreparedLexicalTape},
    parser_punctuation_model::*,
    *,
};
use parser_selection_values::{admit_state, profile, revision_with_text};
use serde_json::{json, Value};

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
fn declared_stable_prefix(text: &str, count: usize, exact: bool, causal: bool) -> u32 {
    if exact && causal {
        [5, 6, 13, 14][count - 1]
    } else {
        text.chars().count() as u32
    }
}
#[test]
fn motivating_causal_text_revision_lineage_is_valid() {
    let mut previous = None;
    for (i, text) in ["Hello ", "Hello, ", "Hello, Travis ", "Hello, Travis."]
        .iter()
        .enumerate()
    {
        let count = i + 1;
        let source = revision_with_text(
            text,
            "causal-lineage",
            &format!("lineage/{count}"),
            i as u64,
            previous.as_ref(),
            Some(declared_stable_prefix(text, count, true, true)),
            LanguageTextFinality::Partial,
        );
        conduit_language::validate_text_revision(
            previous.as_ref(),
            &source,
            0,
            4096,
        )
        .unwrap();
        assert_eq!(*source.stable_prefix(), Some([5, 6, 13, 14][i]));
        previous = Some(source);
    }
}
fn evaluate(
    decoder: &mut parser_selection_driver::Decoder,
    row: &Value,
    partition: &str,
    index: usize,
    exact_motivator: bool,
    causal: bool,
    observations: &mut Vec<Value>,
) -> Value {
    let forms = row["forms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect::<Vec<_>>();
    let part = match partition {
        "previously_observed_dev" => "dev",
        "previously_observed_test" => "test",
        "previously_observed_motivator_not_trained" => "mot",
        _ => "res",
    };
    let track = if causal {
        "causal_commit"
    } else {
        "independent_uncommitted_prefix"
    };
    let text_id = format!("pv3/{part}/{index}/{}", if causal { "c" } else { "q" });
    let pending_through = if exact_motivator {
        3
    } else {
        forms.len() as u64
    };
    let mut previous: Option<(
        LanguageTextRevision,
        PreparedLexicalTape,
        LanguageParserScorerBeam,
    )> = None;
    let mut final_state = None;
    for count in 1..=forms.len() {
        let text = if exact_motivator {
            ["Hello ", "Hello, ", "Hello, Travis ", "Hello, Travis."][count - 1].to_owned()
        } else {
            format!("{} ", forms[..count].join(" "))
        };
        let source = revision_with_text(
            &text,
            &text_id,
            &format!("{text_id}/source/{count}"),
            if causal { count as u64 - 1 } else { 0 },
            previous.as_ref().map(|p| &p.0),
            Some(declared_stable_prefix(
                &text,
                count,
                exact_motivator,
                causal,
            )),
            LanguageTextFinality::Partial,
        );
        let analysis = format!("{text_id}/analysis/{count}");
        let (tape, seeds, coalesced) = if let Some((_, old_tape, old_beam)) = &previous {
            let (tape, beam, coalesced) = decoder.rebase_fresh_revision(
                old_beam,
                old_tape,
                &source,
                &LanguageAnalysisRevisionId::new(analysis.clone()).unwrap(),
            );
            (tape, Some(beam), coalesced)
        } else {
            (
                prepare_lexical_tape(&source, &profile(), None).unwrap(),
                None,
                0,
            )
        };
        assert_eq!(
            tape.tape().tokens().len(),
            count,
            "actual token reconstruction; no reference segmentation supplied"
        );
        let lexical =
            LanguageParserAvailableLexical::new(tape.tape().clone(), count as u64).unwrap();
        let seed_snapshot = seeds.as_ref().map(snapshot);
        let result = if let Some(beam) = seeds {
            decoder.decode_from_beam(&lexical, beam, true)
        } else {
            let initial = decoder.initial(&lexical, &analysis);
            decoder.decode(&lexical, initial, true)
        };
        let mut current = result.beam;
        let observed = observe_commit(
            decoder,
            &mut current,
            &lexical,
            causal.then_some(pending_through),
        );
        let current_state = current.candidate0().parser().state().clone();
        let progress = json!({"text":text,"available_tokens":count,"source_finality":"partial","complete":result.complete,"model_invocations":result.invocations,"decode_ns":result.elapsed_ns,"coalesced_exact_rebased_states":coalesced,"rebased_seed_beam":seed_snapshot,"observation":observed});
        eprintln!("candidate-v3 prefix: {progress}");
        observations.push(progress);
        final_state = Some(current_state.clone());
        if causal {
            previous = Some((source, tape, current));
        }
    }
    // References enter only after all decoding/consensus/commit operations.
    let state = final_state.unwrap();
    let predicted = relations(&state);
    let mut uas = 0;
    let mut las = 0;
    let mut tp = 0;
    let mut fp = 0;
    let mut fn_ = 0;
    let mut predictions = Vec::new();
    for ordinal in 0..forms.len() {
        let head = state.heads()[ordinal];
        let relation = &predicted[ordinal];
        let gold_head = row["heads"][ordinal].as_u64().unwrap();
        let gold_relation = row["relations"][ordinal]
            .as_str()
            .unwrap()
            .split(':')
            .next()
            .unwrap();
        let head_correct = head == gold_head;
        let correct = head_correct && relation == gold_relation;
        uas += u64::from(head_correct);
        las += u64::from(correct);
        let predicted_vocative = relation == "vocative";
        let gold_vocative = gold_relation == "vocative";
        tp += u64::from(predicted_vocative && gold_vocative && head_correct);
        fp += u64::from(predicted_vocative && !(gold_vocative && head_correct));
        fn_ += u64::from(gold_vocative && !(predicted_vocative && head_correct));
        predictions.push(json!({"token":ordinal,"form":forms[ordinal],"head":head,"relation":relation,"gold_head":gold_head,"gold_relation":gold_relation,"las_correct":correct}));
    }
    let mut stability = Vec::new();
    for ordinal in 0..forms.len() {
        let mut prior = None;
        let mut revisions = 0u64;
        let mut contradicted = 0u64;
        let mut first_agreement = None;
        let mut first_commit = None;
        for observation in observations.iter() {
            let prefix = observation["available_tokens"].as_u64().unwrap();
            if prefix <= ordinal as u64 {
                continue;
            }
            let current = &observation["observation"];
            let head = current["after"][0]["heads"][ordinal].as_u64().unwrap();
            let relation = current["after"][0]["relations"][ordinal]
                .as_str()
                .unwrap()
                .to_owned();
            if let Some((old_head, old_relation, was_committed)) = &prior {
                if *old_head != head || *old_relation != relation {
                    revisions += 1;
                    if *was_committed {
                        contradicted += 1;
                    }
                }
            }
            let committed = current["committed"].as_u64().unwrap() > ordinal as u64;
            if current["agreements"][ordinal]["agrees"] == true && first_agreement.is_none() {
                first_agreement = Some(prefix);
            }
            if committed && first_commit.is_none() {
                first_commit = Some(prefix);
            }
            prior = Some((head, relation, committed));
        }
        stability.push(json!({"token":ordinal,"best_edge_revision_count":revisions,"first_survivor_agreement_prefix":first_agreement,"first_actual_commit_prefix":first_commit,"commit_delay_available_tokens":first_commit.map(|prefix|prefix-(ordinal as u64+1)),"committed_edge_later_changed":contradicted}));
    }
    json!({"id":row["id"],"partition":partition,"track":track,"source_reconstruction":if exact_motivator {"exact previously observed motivating revisions"} else {"forms joined with spaces and trailing space; original typography unavailable"},"observations":observations,"predictions":predictions,"stability":stability,"tokens":forms.len(),"uas_correct":uas,"las_correct":las,"vocative_tp":tp,"vocative_fp":fp,"vocative_fn":fn_})
}
fn evaluate_retaining_failure(
    decoder: &mut parser_selection_driver::Decoder,
    row: &Value,
    partition: &str,
    index: usize,
    exact: bool,
    causal: bool,
) -> Value {
    let mut retained = Vec::new();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        evaluate(decoder, row, partition, index, exact, causal, &mut retained)
    })) {
        Ok(value) => value,
        Err(error) => {
            json!({"id":row["id"],"partition":partition,"track":if causal {"causal_commit"} else {"independent_uncommitted_prefix"},"observations":retained,"error":error.downcast_ref::<String>().map(String::as_str).or_else(||error.downcast_ref::<&str>().copied()).unwrap_or("non-string failure"),"tokens":row["forms"].as_array().unwrap().len(),"uas_correct":0,"las_correct":0,"vocative_tp":0,"vocative_fp":0,"vocative_fn":row["relations"].as_array().unwrap().iter().filter(|v|v.as_str().is_some_and(|s|s.split(':').next()==Some("vocative"))).count()})
        }
    }
}
#[test]
fn punctuation_v3_motivating_greeting_actual_source_two_tracks() {
    run_candidate(true, false);
}
#[test]
fn punctuation_v3_retained_dev_test_and_other_reserved_actual_source() {
    run_candidate(false, false);
}
#[test]
fn punctuation_v3_motivating_greeting_actual_source_causal_only() {
    run_candidate(true, true);
}
// Explicit named entrances partition cases; no environment case selection.
fn run_candidate(focused: bool, causal_only: bool) {
    let (artifact, signature) = punctuation_parser_scorer_descriptor();
    let model = parser_model_kernel::ModelDriver::with_descriptor(
        artifact,
        signature,
        PUNCTUATION_PARSER_WEIGHTS,
        prepare_punctuation_parser_scorer,
    );
    let preparation = std::time::Instant::now();
    let mut decoder = parser_selection_driver::Decoder::with_model(model);
    let prepare_ns = preparation.elapsed().as_nanos();
    let mut rows = Vec::new();
    if !focused {
        for (partition, data) in [
            (
                "previously_observed_dev",
                include_str!("../training/ewt_punctuation_v3/dev_annotations.jsonl"),
            ),
            (
                "previously_observed_test",
                include_str!("../training/ewt_punctuation_v3/test_annotations.jsonl"),
            ),
        ] {
            for (i, line) in data.lines().enumerate() {
                let row: Value = serde_json::from_str(line).unwrap();
                for causal in [false, true] {
                    rows.push(evaluate_retaining_failure(
                        &mut decoder,
                        &row,
                        partition,
                        i,
                        false,
                        causal,
                    ));
                }
            }
        }
    }
    let reserved: Vec<Value> = serde_json::from_str(include_str!(
        "../training/ewt_punctuation_v3/reserved_evaluation.json"
    ))
    .unwrap();
    for (i, row) in reserved.iter().enumerate() {
        let exact_motivator = row["id"] == "reserved-punctuation-evaluation/motivator-period";
        if exact_motivator != focused {
            continue;
        }
        let tracks: &[bool] = if causal_only { &[true] } else { &[false, true] };
        for &causal in tracks {
            rows.push(evaluate_retaining_failure(
                &mut decoder,
                row,
                if i < 2 {
                    "previously_observed_motivator_not_trained"
                } else {
                    "reserved_before_training_not_used_to_tune"
                },
                i,
                exact_motivator,
                causal,
            ));
        }
    }
    let motivator = rows.iter().find(|row| {
        row["id"] == "reserved-punctuation-evaluation/motivator-period"
            && row["track"] == "causal_commit"
    });
    let partial_success = motivator.map(|row| {
        let partial = &row["observations"][2]["observation"];
        partial["committed"].as_u64().is_some_and(|n| n >= 3)
            && partial["after"][0]["relations"][2] == "vocative"
            && partial["after"][0]["heads"][2] == 0
    });
    let manifest: Value =
        serde_json::from_str(include_str!("../training/ewt_punctuation_v3/manifest.json")).unwrap();
    let mut totals = std::collections::BTreeMap::<String, [u64; 8]>::new();
    for row in &rows {
        let count = totals
            .entry(format!(
                "{}/{}",
                row["partition"].as_str().unwrap(),
                row["track"].as_str().unwrap()
            ))
            .or_default();
        count[0] += 1;
        count[1] += row["tokens"].as_u64().unwrap();
        for (i, key) in [
            "uas_correct",
            "las_correct",
            "vocative_tp",
            "vocative_fp",
            "vocative_fn",
        ]
        .iter()
        .enumerate()
        {
            count[i + 2] += row[key].as_u64().unwrap_or(0);
        }
        count[7] += u64::from(!row["error"].is_null());
    }
    let numerical=decoder.model.receipts().iter().map(|r|json!({"invocation":r.invocation,"model_content":r.model_content,"input_digest":r.input,"output_digest":r.output,"work_units":r.work_units})).collect::<Vec<_>>();
    let report = json!({"scope":if causal_only {"motivating greeting causal track only; independent negative evidence retained separately, not rerun"} else if focused {"motivating greeting only; two tracks; four exact revisions including pre-final space and later period"} else {"all retained DEV/TEST and other reserved cases; motivating period case covered separately"},"profile":"explicit unaccepted punctuation candidate; fresh-per-text-revision scoring + exact coalescing; no default replacement","inference_profile":{"score_reset":"zero for every retained seed at each new text revision; only current continuations scored","rebase":"every active state; exact committed lexical choices retained; only exact state+choice duplicates coalesced after existing Source rank","commit":"causal track waits until actual contiguous stable prefix covers named pending consumer; no early root/comma locking","quality_track":"independent prefixes without commitment; no committed-state conditioning"},"manifest":manifest,"prepare_ns":prepare_ns,"rows":rows,"partition_totals_columns":["sentences","tokens","uas_correct","las_correct","vocative_tp","vocative_fp","vocative_fn","errors"],"partition_totals":totals,"numerical_receipts":numerical,"model_invocations":decoder.model.invocations(),"committed_model_receipts":decoder.model.receipts().len(),"motivator_partial_commit_success":partial_success,"limitations":["four-token finite profile","DEV/TEST and motivator previously observed; no newly blind accuracy claim","reserved outputs must not be used to retune this candidate","same Source legal-action/ranking/four-survivor consensus policy, no pruning changes","test-only current-beam orchestration, not an installed live editor or acoustic playback","negative conditioned TEST regression retained in training manifest"]});
    let path = std::env::var_os("CONDUIT_PARSER_PUNCTUATION_V3_REPORT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(if causal_only {
                "conduit-parser-punctuation-v3-causal-report.json"
            } else if focused {
                "conduit-parser-punctuation-v3-greeting-report.json"
            } else {
                "conduit-parser-punctuation-v3-retained-report.json"
            })
        });
    std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    eprintln!("candidate-v3 retained report: {}", path.display());
    assert!(
        rows.iter().all(|row| row["error"].is_null()),
        "actual Source evaluation failed; retain report"
    );
    assert!(
        !focused || partial_success == Some(true),
        "candidate does not establish committed pre-final vocative; retain report and original gap"
    );
}
