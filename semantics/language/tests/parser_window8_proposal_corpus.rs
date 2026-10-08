//! Successor evaluation keeps original proposal origins and all supplied rows.
#![cfg(feature = "parser-model-selection")]
#[path = "common/window8_proposal_candidate.rs"]
mod candidate;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
#[path = "common/parser_planned_runtime.rs"]
#[allow(dead_code)]
mod planned;
#[path = "common/window8_proposed_lexical.rs"]
mod proposed;
use conduit_core::*;
use conduit_language::{parser_window8_program_bank::*, *};
use conduit_plot::rust_binding::{NativeRustBinding, PreparedNativeFamilyLimits};
use serde_json::{json, Value};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn failure(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
fn decode_row(
    bank: &Window8ProgramBank,
    execution: &mut planned::Execution,
    owner:&mut conduit_language::lexical_proposer_port::token_producer::revision::PreparedRevisionProducer,
    row: &Value,
    model_identity: &[u8],
    invocations: &mut u16,
) -> Result<Value, String> {
    let text = row["text"].as_str().ok_or("missing original text")?;
    let original_id = row["id"]
        .as_str()
        .ok_or("missing full original row identity")?;
    let mut identity_material = (original_id.len() as u64).to_le_bytes().to_vec();
    identity_material.extend_from_slice(original_id.as_bytes());
    identity_material.extend_from_slice(text.as_bytes());
    let identity = hex(&semantic_digest(
        "language/proposal-window8-evaluation-input@1",
        &identity_material,
    ));
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new(identity.clone()).map_err(failure)?,
            LanguageId::new("language/en".into()).map_err(failure)?,
            LanguageTextRevisionId::new(identity).map_err(failure)?,
            text.into(),
        )
        .map_err(failure)?,
        None,
        LinguisticDerivationProvenance::deterministic_rule(
            "proposal-corpus-evaluation".into(),
            "original-supplied-text".into(),
        )
        .map_err(failure)?,
        0,
        None,
    )
    .map_err(failure)?;
    let original = owner
        .propose(&source.clone().encode().map_err(failure)?, None, 0)
        .map_err(failure)?;
    let lexical = proposed::ProposedLexical::prepare(bank, &original)?;
    if lexical.proposed.clone().encode().map_err(failure)?
        != lexical.original.canonical_proposed_tape()
    {
        return Err("full original proposed-tape custody mismatch".into());
    }
    let actual = lexical
        .lexical
        .tape()
        .tokens()
        .iter()
        .map(|token| token.surface().as_str())
        .collect::<Vec<_>>();
    let expected = row["forms"]
        .as_array()
        .ok_or("missing original forms")?
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(format!(
            "original tokenization differs: actual={actual:?}, expected={expected:?}"
        ));
    }
    let material = [
        original.canonical_proposed_tape(),
        &source.clone().encode().map_err(failure)?,
        model_identity,
    ]
    .concat();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new(hex(&semantic_digest(
            "language/proposal-window8-evaluation-analysis@1",
            &material,
        )))
        .map_err(failure)?,
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .map_err(failure)?;
    let default = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).map_err(failure)?,
    )
    .map_err(failure)?;
    let initial = bank
        .initialize(
            &LanguageParserWindow8Begin::new(
                basis.clone(),
                default.clone(),
                *lexical.lexical.token_count(),
            )
            .map_err(failure)?,
        )
        .map_err(failure)?;
    let empty = |identity, active| {
        LanguageParserWindow8RawHypothesis::new(
            active,
            [0; 8],
            identity,
            0,
            0,
            initial.state().clone(),
        )
    };
    let mut beam = LanguageParserWindow8RawBeam::new(
        empty(0, true).map_err(failure)?,
        empty(1, false).map_err(failure)?,
        empty(2, false).map_err(failure)?,
        empty(3, false).map_err(failure)?,
    )
    .map_err(failure)?;
    let classes = (0..76)
        .map(|code| bank.class(code, &default).map_err(failure))
        .collect::<Result<Vec<_>, _>>()?;
    let mut identity = 4;
    let mut epochs = Vec::new();
    let started = std::time::Instant::now();
    for epoch in 0..32 {
        let mut next = LanguageParserWindow8RawBeam::new(
            empty(0, false).map_err(failure)?,
            empty(1, false).map_err(failure)?,
            empty(2, false).map_err(failure)?,
            empty(3, false).map_err(failure)?,
        )
        .map_err(failure)?;
        for prior in [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ]
        .into_iter()
        .filter(|v| *v.active())
        {
            let state = bank.admit_state(prior.state()).map_err(failure)?;
            if bank.complete(&state).map_err(failure)? {
                next = bank.merge(next, prior.clone()).map_err(failure)?;
                continue;
            }
            let context = bank.context(&state, &basis).map_err(failure)?;
            let unread = *prior.state().unread() as usize;
            let choose = unread < *lexical.lexical.token_count() as usize
                && *prior.selected() == unread as u64;
            let alternatives = if choose {
                *lexical.projection.tokens()[unread].count()
            } else {
                1
            };
            for choice in 0..alternatives {
                let mut choices = *prior.choices();
                if choose {
                    choices[unread] = choice;
                }
                let (query, features) = lexical.features(bank, &state, &basis, choices)?;
                let frontier = bank
                    .choice_frontier(
                        LanguageParserWindow8ChoiceQuery::new(
                            query.raw().clone(),
                            *prior.choices(),
                            *prior.selected(),
                        )
                        .map_err(failure)?,
                    )
                    .map_err(failure)?;
                let scores = execution.infer(
                    u64::from(*invocations),
                    &features.raw().clone().into_structured().map_err(failure)?,
                );
                *invocations = invocations.checked_add(1).ok_or("invocation overflow")?;
                let StructuredInfoValueShape::Collection(scores) = scores.shape() else {
                    return Err("numeric output is not original collection".into());
                };
                if scores.len() != 76 {
                    return Err("numeric output count".into());
                }
                for (class, value) in classes.iter().zip(scores) {
                    let proposal = context.propose(class).map_err(failure)?;
                    if !proposal.proposal().accepted() {
                        continue;
                    }
                    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                        return Err("numeric score is not original I64".into());
                    };
                    let score = i64::from_le_bytes(bytes.try_into().map_err(failure)?);
                    let raw = bank
                        .score_advance(
                            LanguageParserWindow8RawAdvance::new(
                                choices,
                                identity,
                                proposal.proposal().clone(),
                                *prior.score(),
                                *frontier.count(),
                                score,
                            )
                            .map_err(failure)?,
                        )
                        .map_err(failure)?;
                    identity += 1;
                    next = bank.merge(next, raw).map_err(failure)?;
                }
            }
        }
        beam = next;
        let complete = [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ]
        .into_iter()
        .filter(|v| *v.active())
        .map(|v| {
            bank.admit_state(v.state())
                .and_then(|state| bank.complete(&state))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?
        .into_iter()
        .all(|v| v);
        epochs.push(json!({"epoch":epoch,"elapsed_nanos":started.elapsed().as_nanos().to_string(),"completed_model_calls":invocations,"beam_canonical":hex(&beam.clone().encode().map_err(failure)?),"all_complete":complete}));
        if complete {
            break;
        }
    }
    if !*beam.candidate0().active() {
        return Err("no active preferred candidate".into());
    }
    let preferred = beam.candidate0();
    let state = bank.admit_state(preferred.state()).map_err(failure)?;
    let count = *lexical.lexical.token_count() as usize;
    let pos = (0..count)
        .map(|i| lexical.projection.tokens()[i].codes()[preferred.choices()[i] as usize])
        .collect::<Vec<_>>();
    let relations = [
        state.state().relation0(),
        state.state().relation1(),
        state.state().relation2(),
        state.state().relation3(),
        state.state().relation4(),
        state.state().relation5(),
        state.state().relation6(),
        state.state().relation7(),
    ];
    let predicted=(0..count).map(|i|json!({"head":state.state().heads()[i],"base":format!("{:?}",relations[i].base()).to_lowercase()})).collect::<Vec<_>>();
    Ok(
        json!({"id":row["id"],"complete":bank.complete(&state).map_err(failure)?,"pos":pos,"predicted":predicted,"original_revision":hex(&source.encode().map_err(failure)?),"original_proposed_tape":hex(original.canonical_proposed_tape()),"epochs":epochs,"qualified_fact_claim":false,"commit_claim":false}),
    )
}
fn metrics(rows: &[Value], receipts: &[Value]) -> Value {
    let mut tokens = 0usize;
    let mut heads = 0;
    let mut labels = 0;
    let mut pos = 0;
    let mut exact = 0;
    let mut refusals = 0;
    let (mut tp, mut fp, mut fn_) = (0, 0, 0);
    for (row, receipt) in rows.iter().zip(receipts) {
        let count = row["forms"].as_array().unwrap().len();
        tokens += count;
        let refused = receipt.get("refusal").is_some();
        refusals += usize::from(refused);
        let mut matched = !refused && receipt["complete"] == true;
        for i in 0..count {
            let gold = row["relations"][i]
                .as_str()
                .unwrap()
                .split(':')
                .next()
                .unwrap();
            let predicted = if refused {
                None
            } else {
                receipt["predicted"][i]["base"].as_str()
            };
            let head = !refused && receipt["predicted"][i]["head"] == row["heads"][i];
            let relation = head && predicted == Some(gold);
            let tag = !refused && receipt["pos"][i] == row["pos"][i];
            heads += usize::from(head);
            labels += usize::from(relation);
            pos += usize::from(tag);
            matched &= head && relation && tag;
            let p = predicted == Some("vocative");
            let g = gold == "vocative";
            let hit = p && g && head;
            tp += usize::from(hit);
            fp += usize::from(p && !hit);
            fn_ += usize::from(g && !hit);
        }
        exact += usize::from(matched);
    }
    json!({"tokens":tokens,"correct_heads":heads,"correct_base_labels":labels,"correct_pos":pos,"exact_graphs":exact,"refused_rows":refusals,"uas":heads as f64/tokens as f64,"base_las":labels as f64/tokens as f64,"pos_accuracy":pos as f64/tokens as f64,"vocative_tp":tp,"vocative_fp":fp,"vocative_fn":fn_,"vocative_precision":if tp+fp==0 {None}else{Some(tp as f64/(tp+fp) as f64)},"vocative_recall":if tp+fn_==0 {None}else{Some(tp as f64/(tp+fn_) as f64)},"token_scope":"all supplied original tokens, including refused rows and punctuation"})
}
#[test]
#[ignore = "actual pinned successor Source/Plan model evaluation"]
fn actual_proposal_window8_corpus() {
    let input = std::env::var("PROPOSAL_EVALUATION_ROWS").expect("explicit rows");
    let output = std::env::var("PROPOSAL_EVALUATION_OUTPUT").expect("disjoint output");
    assert!(
        !std::path::Path::new(&output).exists(),
        "preserve prior receipts"
    );
    let bytes = std::fs::read(input).unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    let rows: Vec<Value> = if document.is_array() {
        serde_json::from_value(document).unwrap()
    } else {
        serde_json::from_value(document["rows"].clone()).unwrap()
    };
    assert!(
        !rows.is_empty() && rows.len() <= 31,
        "finite per-process chunk; aggregate retains full denominator"
    );
    let mut candidate = candidate::prepare();
    assert_eq!(
        candidate
            .selected
            .declaration()
            .source_contract()
            .feature_contract,
        candidate.contracts.feature_contract
    );
    let mut execution = planned::prepare_proposal_window8_v2_source_with_inference_budget(
        candidate.scorer.clone(),
        candidate::numeric_source(&candidate),
        "window8-proposal-v2-learned-model",
        16127,
    );
    let bank = Window8ProgramBank::prepare_proposal_v2_native_evaluator(
        PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 256,
            maximum_input_bytes: 262144,
            maximum_retained_bytes: 256 * 1024 * 1024,
            maximum_preparation_peak_bytes: 512 * 1024 * 1024,
            maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
        },
        Window8SourcePreparationLimits {
            maximum_retained_bytes: 512 * 1024 * 1024,
            maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
            maximum_input_bytes: 262144,
        },
    )
    .unwrap();
    let declaration = candidate.selected.declaration();
    let model_identity = [
        declaration.signature().clone().encode().unwrap(),
        format!("{:?}", declaration.artifact()).into_bytes(),
        declaration.training_manifest().to_vec(),
        declaration.feature_abi().as_bytes().to_vec(),
        declaration.canonical_proposer_definition().to_vec(),
        format!("{:?}", declaration.source_contract()).into_bytes(),
    ]
    .concat();
    let mut calls = 0;
    let mut receipts = Vec::new();
    let started = std::time::Instant::now();
    for row in &rows {
        let result = decode_row(
            &bank,
            &mut execution,
            &mut candidate.owner,
            row,
            &model_identity,
            &mut calls,
        );
        receipts.push(match result {
            Ok(value) => value,
            Err(refusal) => json!({"id":row["id"],"refusal":refusal}),
        });
        std::fs::write(&output,serde_json::to_vec_pretty(&json!({"input_digest":hex(&semantic_digest("language/proposal-evaluation-rows@1",&bytes)),"supplied_rows":rows.len(),"observed_rows":receipts.len(),"metrics":metrics(&rows[..receipts.len()],&receipts),"elapsed_nanos":started.elapsed().as_nanos().to_string(),"actual_model_calls":calls,"receipts":receipts,"whole_parser_bound_claim":false,"heldout_claim":false,"stable_fact_claim":false})).unwrap()).unwrap();
    }
}

#[test]
fn refused_rows_keep_original_token_and_vocative_denominators() {
    let rows = vec![
        json!({"forms":["I"],"heads":[8],"relations":["root"],"pos":[7]}),
        json!({"forms":["Morgan"],"heads":[0],"relations":["vocative"],"pos":[11]}),
    ];
    let receipts = vec![
        json!({"complete":true,"predicted":[{"head":8,"base":"root"}],"pos":[7]}),
        json!({"refusal":"original tokenization differs"}),
    ];
    let result = metrics(&rows, &receipts);
    assert_eq!(result["tokens"], 2);
    assert_eq!(result["uas"], 0.5);
    assert_eq!(result["base_las"], 0.5);
    assert_eq!(result["pos_accuracy"], 0.5);
    assert_eq!(result["vocative_fn"], 1);
    assert_eq!(result["refused_rows"], 1);
    assert_eq!(result["exact_graphs"], 1);
}
