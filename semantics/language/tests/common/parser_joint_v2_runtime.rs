use super::parser_joint_flows::Pipelines;
use super::{fixture, joint, parser_kernel};
use conduit_ai::integer_masked_rank::integer_masked_top_k;
use conduit_core::*;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
use fixture::{count, field, field_type, number, record};
pub trait Observer {
    fn initialize(
        &mut self,
        _lexical: &LanguageParserJointLexical,
        initial: LanguageParserState,
    ) -> LanguageParserState {
        initial
    }
    fn waiting(
        &mut self,
        _lexical: &LanguageParserJointLexical,
        _state: &LanguageParserState,
    ) -> bool {
        false
    }
    fn seed_metadata(&self) -> ([u64; 4], u64) {
        ([0; 4], 0)
    }
    fn branch_entry(&self) -> Option<(&'static str, String)> {
        None
    }
    fn branch_input(
        &mut self,
        query: LanguageParserJointBranchQuery,
    ) -> Result<StructuredInfoValue, conduit_plot::rust_binding::NativeBindingRefusal> {
        query.into_structured()
    }
    fn planned_scores(&mut self, _features: &LanguageParserV2ModelFeatures) -> Option<Vec<i64>> {
        None
    }
    fn snapshot(&mut self, _beam: &LanguageParserJointRuntimeBeam) -> serde_json::Value {
        serde_json::Value::Null
    }
}
#[allow(dead_code)]
struct FullTape;
impl Observer for FullTape {}
#[allow(dead_code)]
pub fn evaluate_rows(
    selected: Option<&str>,
    rows: Vec<joint::Sentence>,
    native_inputs: Vec<Result<LanguageParserJointLexical, &'static str>>,
) {
    evaluate_observed(selected, rows, native_inputs, &mut FullTape);
}
pub fn evaluate_observed(
    selected: Option<&str>,
    rows: Vec<joint::Sentence>,
    native_inputs: Vec<Result<LanguageParserJointLexical, &'static str>>,
    observer: &mut impl Observer,
) {
    let started = std::time::Instant::now();
    let expected_complete = native_inputs.iter().filter(|input| {
        matches!(input, Ok(lexical) if matches!(lexical.tape().source().finality(), LanguageTextFinality::Final))
    }).count() as u64;
    let mut exclusions = std::collections::BTreeMap::<&str, u64>::new();
    let mut f = fixture::Fixture::new();
    let mut complete = f.prepare("language-parser-decode-complete");
    // Every entry expands its own actual top-level Source graph. This harness
    // connects Flow invocations and hosted numerical inference; it defines no
    // parser transition, legality, lexical selection, score accumulation or rank.
    let entries = [
        "language-parser-joint-branch",
        "language-parser-v2-pos",
        "language-parser-v2-model-features",
        "language-parser-legal-mask",
        "language-parser-score-proposal",
        "language-parser-transition",
        "language-parser-joint-expansion",
        "language-parser-joint-runtime-merge",
    ];
    let blueprints = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            eprintln!(
                "joint Source prepare {entry} elapsed_ms={}",
                started.elapsed().as_millis()
            );
            let source = if (3..=5).contains(&i) {
                fixture::parser_source()
            } else if i == 1 || i == 2 {
                joint::pos_source()
            } else {
                joint::runtime_source()
            };
            if i == 0 {
                if let Some((protected_entry, extra_source)) = observer.branch_entry() {
                    return parser_kernel::Blueprint::prepare(
                        format!("{source}\n{extra_source}"),
                        protected_entry,
                    );
                }
            }
            parser_kernel::Blueprint::prepare(source, entry)
        })
        .collect::<Vec<_>>();
    let source_preparation_ms = started.elapsed().as_millis();
    let mut scorer = joint::scorer();
    let mut pipelines = None;
    let mut graphs = Vec::new();
    let mut counts = [0u64; 10]; // sentences,tokens,POS,ambiguous,ambiguousPOS,UAS,LAS,complete,vocGold,vocCorrect
    let profile: std::collections::BTreeMap<String, Vec<u64>> =
        serde_json::from_slice(joint::PROFILE).unwrap();
    let mut preparation_ms = 0;
    let mut decode_ms = 0;
    let mut decode_max_ms = 0;
    let mut identities = 1u64;
    let mut accepted_rows = 0usize;
    for (row, lexical) in rows.iter().zip(native_inputs) {
        let lexical = match lexical {
            Ok(lexical) => lexical,
            Err(reason) => {
                *exclusions.entry(reason).or_default() += 1;
                continue;
            }
        };
        // Fresh ordinary Plan/Play scopes after eight analyses keep each finite
        // prepared Sign store within its declared envelope. No kernel reset,
        // trace eviction, Value-to-Flow lift or reused old temporal receipt.
        if accepted_rows.is_multiple_of(8) {
            let start = std::time::Instant::now();
            pipelines = Some(Pipelines::new(&blueprints, accepted_rows / 8));
            preparation_ms += start.elapsed().as_millis();
        }
        accepted_rows += 1;
        let pipeline = pipelines.as_mut().unwrap();
        let initial = observer.initialize(&lexical, joint::initial(&mut f, &lexical, &scorer));
        eprintln!(
            "joint native decode {} prepared_ms={preparation_ms}",
            row.id
        );
        let (seed_choices, seed_selected) = observer.seed_metadata();
        let seed = LanguageParserJointRuntimeHypothesis::new(
            LanguageParserJointHypothesis::new(
                seed_choices,
                LanguageParserHypothesis::new(true, identities, 0, initial.clone()).unwrap(),
            )
            .unwrap(),
            seed_selected,
        )
        .unwrap();
        identities += 1;
        let mut beam = pipeline.merge(joint::empty(&initial), seed);
        let is_complete =
            |state: &LanguageParserState,
             evaluator: &mut conduit_plot::PreparedPortableExpressionEvaluator| {
                let output = evaluator
                    .evaluate(
                        &state
                            .clone()
                            .into_structured()
                            .unwrap()
                            .canonical_bytes()
                            .unwrap(),
                    )
                    .unwrap();
                assert!(output == [0] || output == [1]);
                output == [1]
            };
        let decode_started = std::time::Instant::now();
        for _ in 0..12 {
            let old = joint::slots(&beam).map(Clone::clone);
            let mut next = joint::empty(&initial);
            let mut any_unfinished = false;
            for prior in old
                .into_iter()
                .filter(|c| *c.hypothesis().parser().active())
            {
                let state = prior.hypothesis().parser().state();
                if observer.waiting(&lexical, state) || is_complete(state, &mut complete) {
                    next = pipeline.merge(next, prior);
                    continue;
                }
                any_unfinished = true;
                let choices = if *state.unread() == *state.token_count() {
                    vec![prior.clone()]
                } else {
                    let mut choices = Vec::new();
                    for choice in 0..4 {
                        let query = LanguageParserJointBranchQuery::new(
                            choice,
                            prior.clone(),
                            lexical.clone(),
                        )
                        .unwrap();
                        let input = match observer.branch_input(query) {
                            Ok(input) => input,
                            Err(_) => continue, // The Source-native policy refusal is retained by the observer.
                        };
                        let output = LanguageParserJointBranchResult::from_structured(
                            pipeline.call(0, &input),
                        )
                        .unwrap();
                        if *output.accepted() {
                            choices.push(
                                LanguageParserJointRuntimeHypothesis::from_structured(
                                    joint::retype(
                                        &LanguageParserJointRuntimeHypothesis::semantic_type()
                                            .unwrap(),
                                        &output.proposal().clone().into_structured().unwrap(),
                                    ),
                                )
                                .unwrap(),
                            );
                        }
                    }
                    choices
                };
                for chosen in choices {
                    let state = chosen.hypothesis().parser().state();
                    let query = LanguageParserV2ChoiceQuery::new(
                        *chosen.hypothesis().choices(),
                        *lexical.tape().source().sequence(),
                        pipeline.flows[2].sequence,
                        LanguageParserAvailableLexical::new(
                            lexical.tape().clone(),
                            *lexical.token_count(),
                        )
                        .unwrap(),
                        state.clone(),
                    )
                    .unwrap();
                    let features = LanguageParserV2ModelFeatures::from_structured(
                        pipeline.call(2, &query.into_structured().unwrap()),
                    )
                    .unwrap();
                    let (scores, evidence) = scorer.score(
                        features.indices(),
                        features
                            .clone()
                            .into_structured()
                            .unwrap()
                            .semantic_digest()
                            .unwrap(),
                    );
                    assert_eq!(
                        evidence.artifact_identity,
                        scorer.artifact.content_identity()
                    );
                    let scores = if let Some(planned) = observer.planned_scores(&features) {
                        assert_eq!(
                            planned, scores,
                            "ordinary selected model and pinned arithmetic differ"
                        );
                        planned
                    } else {
                        scores
                    };

                    let mask_ty = LanguageParserMaskQuery::semantic_type().unwrap();
                    let mask_query = record(
                        &mask_ty,
                        vec![
                            ("basis", state.basis().clone().into_structured().unwrap()),
                            (
                                "state",
                                joint::retype(
                                    field_type(&mask_ty, "state"),
                                    &state.clone().into_structured().unwrap(),
                                ),
                            ),
                        ],
                    );
                    let mask =
                        LanguageParserLegalMask::from_structured(pipeline.call(3, &mask_query))
                            .unwrap();
                    let ranked = integer_masked_top_k(&scores, mask.allowed(), 4).unwrap();
                    for class in ranked {
                        let ty = LanguageParserScoredClass::semantic_type().unwrap();
                        let scored = record(
                            &ty,
                            vec![
                                ("basis", state.basis().clone().into_structured().unwrap()),
                                (
                                    "state",
                                    joint::retype(
                                        field_type(&ty, "state"),
                                        &state.clone().into_structured().unwrap(),
                                    ),
                                ),
                                ("class", number(field_type(&ty, "class"), class as u64)),
                                (
                                    "score",
                                    StructuredInfoValue::leaf(
                                        field_type(&ty, "score").clone(),
                                        scores[class].to_le_bytes().to_vec(),
                                    )
                                    .unwrap(),
                                ),
                                (
                                    "default_relation",
                                    state.relation0().clone().into_structured().unwrap(),
                                ),
                            ],
                        );
                        let scored = LanguageParserScoredClass::from_structured(scored).unwrap();
                        let proposal = LanguageParserScoredProposal::from_structured(
                            pipeline.call(4, &scored.into_structured().unwrap()),
                        )
                        .unwrap();
                        let result = LanguageParserResult::from_structured(
                            pipeline
                                .call(5, &proposal.request().clone().into_structured().unwrap()),
                        )
                        .unwrap();
                        assert!(
                            *result.accepted(),
                            "source legal mask and transition disagree"
                        );
                        let expansion = LanguageParserJointExpansion::new(
                            identities,
                            chosen.clone(),
                            result,
                            scores[class],
                        )
                        .unwrap();
                        identities += 1;
                        let raw = pipeline.call(6, &expansion.into_structured().unwrap());
                        let admitted =
                            LanguageParserJointRuntimeHypothesis::from_structured(joint::retype(
                                &LanguageParserJointRuntimeHypothesis::semantic_type().unwrap(),
                                &raw,
                            ))
                            .unwrap();
                        next = pipeline.merge(next, admitted);
                    }
                }
            }
            joint::admit(&next, &lexical, initial.basis(), pipeline.flows[7].sequence);
            beam = next;
            if !any_unfinished {
                break;
            }
            if joint::slots(&beam)
                .iter()
                .all(|c| !*c.hypothesis().parser().active())
            {
                break;
            }
        }
        let elapsed = decode_started.elapsed().as_millis();
        decode_ms += elapsed;
        decode_max_ms = decode_max_ms.max(elapsed);
        joint::admit(&beam, &lexical, initial.basis(), pipeline.flows[7].sequence);
        let session = observer.snapshot(&joint::admit(
            &beam,
            &lexical,
            initial.basis(),
            pipeline.flows[7].sequence,
        ));
        counts[0] += 1;
        counts[1] += row.pos.len() as u64;
        let best = beam.candidate0();
        if *best.hypothesis().parser().active() {
            let state = best.hypothesis().parser().state();
            let query = LanguageParserV2ChoiceQuery::new(
                *best.hypothesis().choices(),
                *lexical.tape().source().sequence(),
                pipeline.flows[1].sequence,
                LanguageParserAvailableLexical::new(lexical.tape().clone(), *lexical.token_count())
                    .unwrap(),
                state.clone(),
            )
            .unwrap();
            let pos = LanguageParserV2PosContext::from_structured(
                pipeline.call(1, &query.clone().into_structured().unwrap()),
            )
            .unwrap();
            counts[7] += is_complete(state, &mut complete) as u64;
            let mut predicted = Vec::new();
            for i in 0..row.pos.len() {
                counts[2] += (pos.pos()[i] == row.pos[i]) as u64;
                if profile[&row.forms[i]].len() > 1 {
                    counts[3] += 1;
                    counts[4] += (pos.pos()[i] == row.pos[i]) as u64;
                }
                let gold_base = row.relations[i].split(':').next().unwrap();
                counts[8] += (gold_base == "vocative") as u64;
                let projection = f.project(
                    &joint::retype(
                        f.ty("LanguageParserNumericState"),
                        &state.clone().into_structured().unwrap(),
                    ),
                    i as u64,
                );
                let StructuredInfoValueShape::Variant { tag, payload } = projection.shape() else {
                    panic!("arc projection")
                };
                if tag == "assigned" {
                    let canonical = joint::admit_arc(payload);
                    let head = count(field(payload, "head"));
                    let label = fixture::tag(field(field(payload, "relation"), "base"));
                    counts[5] += (head == row.heads[i]) as u64;
                    counts[6] += (head == row.heads[i] && label == gold_base) as u64;
                    counts[9] += (head == row.heads[i]
                        && label == "vocative"
                        && gold_base == "vocative") as u64;
                    predicted.push(serde_json::json!({"dependent":i,"head":head,"base":label,"canonical_arc_identity":joint::hex(canonical.clone().into_structured().unwrap().semantic_digest().unwrap()),"canonical_arc_bytes":canonical.into_structured().unwrap().canonical_bytes().unwrap()}));
                }
            }
            if selected.is_some() {
                let receipt = serde_json::json!({"session":session,"id":row.id,"text":joint::sentence_text(row),"acquisition_history":joint::acquisition_history(&row.id),"model_content_identity":joint::hex(scorer.artifact.content_identity()),"model_signature_identity":joint::hex(scorer.signature.semantic_digest().unwrap()),"feature_class_contract_identity":joint::hex(semantic_digest("language/parser-v2-scorer-encoding@1", include_bytes!("../../parser_scorer_v2.conduit"))),"text_identity":state.basis().text().get(),"source_revision":state.basis().source_revision().get(),"analysis_revision":state.basis().analysis_revision().get(),"source_material_bytes":lexical.tape().source().clone().into_structured().unwrap().canonical_bytes().unwrap(),"lexical_tape_bytes":lexical.tape().clone().into_structured().unwrap().canonical_bytes().unwrap(),"lexical_pos_codes":pos.pos(),"basis":state.basis().clone().into_structured().unwrap().canonical_bytes().unwrap(),"choices":best.hypothesis().choices(),"predicted":predicted,"complete":is_complete(state,&mut complete)});
                eprintln!("JOINT_NATIVE_GRAPH {receipt}");
                graphs.push(receipt);
            }
        } else {
            counts[3] += row.forms.iter().filter(|w| profile[*w].len() > 1).count() as u64;
        }
        if accepted_rows.is_multiple_of(8) {
            eprintln!("joint native sentences={accepted_rows}");
        }
    }
    let memory = std::fs::read_to_string("/proc/self/status")
        .ok()
        .map(|status| {
            status
                .lines()
                .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
                .collect::<Vec<_>>()
                .join("; ")
        });
    let flow_timing = pipelines.as_ref().map(|p| p.flows.iter().zip(entries).map(|(f,name)| serde_json::json!({"entry":name,"calls":f.calls,"nanos":f.nanos,"maximum_nanos":f.maximum_nanos})).collect::<Vec<_>>());
    let mut metrics = serde_json::json!({"host_memory":memory,"last_scope_flow_timing":flow_timing,"proof":"actual Source Flow POS/branch/features/mask/proposal/transition/score/width4 merge with recursive native admission and exact hosted learned artifact","profile":"TRAIN-frequency57 plus seven separately reviewed lexical forms; four-token complete immutable tape; blind learned joint decode; teaching cases are not heldout","conditional_gold_pos":false,"selected_occurrence":selected,"eligible_heldout_sentences":rows.len(),"canonical_exclusions":exclusions,"sentences":counts[0],"tokens":counts[1],"pos_correct":counts[2],"ambiguous_tokens":counts[3],"ambiguous_pos_correct":counts[4],"uas_correct":counts[5],"universal_base_las_correct":counts[6],"complete_sentences":counts[7],"expected_complete_final_inputs":expected_complete,"vocative_gold":counts[8],"vocative_correct":counts[9],"source_preparation_ms":source_preparation_ms,"ordinary_plan_preparation_ms":preparation_ms,"decode_ms":decode_ms,"decode_max_ms":decode_max_ms,"elapsed_ms":started.elapsed().as_millis(),"model_content_identity":joint::hex(scorer.artifact.content_identity()),"limitations":"finite full-tape beam proof; no incremental revision, stabilization/commitment latency or broad English accuracy; score outputs are not calibrated lexical confidence"});
    if selected == Some("retained-stream") {
        metrics["proof"] = serde_json::json!("actual retained Source Flow partial-prefix learned decode with Source wait/reset and joint lexical/arc agreement observations");
        metrics["profile"] = serde_json::json!("pinned v2 four-token profile; one reviewed Travis/Hello stream; immutable selected model; execution mode recorded per snapshot; no per-revision new kernel");
        metrics["limitations"] = serde_json::json!("bounded orchestration; model execution mode is recorded in each session snapshot; no complete production aggregate Plan, Source stabilization advance, or committed-prefix rebase; provisional outputs are not committed truth");
        metrics["model_bytes"] = serde_json::json!(joint::BYTES.len());
        metrics["compute_ms_per_token"] =
            serde_json::json!(decode_ms as f64 / counts[1].max(1) as f64);
    }
    eprintln!("JOINT_NATIVE_METRICS {metrics}");
    if let Ok(path) = std::env::var("CONDUIT_PARSER_JOINT_V2_METRICS_OUTPUT") {
        std::fs::write(path, serde_json::to_string_pretty(&metrics).unwrap()).unwrap();
    }
    if let Ok(path) = std::env::var("CONDUIT_PARSER_JOINT_V2_GRAPHS_OUTPUT") {
        std::fs::write(path, serde_json::to_string_pretty(&graphs).unwrap()).unwrap();
    }
    if selected.is_some() && !matches!(selected, Some("reviewed-punctuation" | "retained-stream")) {
        assert_eq!(counts[0], rows.len() as u64);
        assert_eq!(
            counts[1],
            rows.iter().map(|r| r.pos.len() as u64).sum::<u64>()
        );
        assert_eq!(counts[2], counts[1]);
        assert_eq!(counts[4], counts[3]);
        assert_eq!(counts[5], counts[1]);
        assert_eq!(counts[6], counts[1]);
        assert_eq!(counts[7], expected_complete);
        assert_eq!(counts[9], counts[8]);
    }
}
