use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::PreparedPortableExpressionEvaluator;
#[allow(dead_code)]
mod admitted {
    include!(concat!(env!("OUT_DIR"), "/parser_admission_types.rs"));
}
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/scorer_model.rs"]
mod scorer_model;
use fixture::*;
#[derive(serde::Deserialize)]
struct Sentence {
    id: String,
    pos: Vec<u64>,
    heads: Vec<u64>,
    relations: Vec<String>,
}
fn retype(ty: &StructuredInfoType, value: &StructuredInfoValue) -> StructuredInfoValue {
    match (ty.shape(), value.shape()) {
        (
            StructuredInfoTypeShape::Record { fields, .. },
            StructuredInfoValueShape::Record(values),
        ) => record(
            ty,
            fields
                .iter()
                .map(|f| {
                    (
                        f.name(),
                        retype(
                            f.value_type(),
                            values
                                .iter()
                                .find(|v| v.name() == f.name())
                                .unwrap()
                                .value(),
                        ),
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}
fn source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        include_str!("../identity.conduit"),
        include_str!("../types.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_beam.conduit"),
        include_str!("../parser_scorer.conduit"),
        include_str!("../parser_mask.conduit")
    )
}
fn basis(f: &Fixture, id: &str) -> StructuredInfoValue {
    let digest = semantic_digest("ud/ewt-sentence-occurrence@1", id.as_bytes());
    let identity = digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let ty = f.ty("LanguageParserBasis");
    record(
        ty,
        vec![
            ("text", text(field_type(ty, "text"), &identity)),
            (
                "source_revision",
                text(
                    field_type(ty, "source_revision"),
                    "fa024f43dc5da3c5ac02563bc9bd0e974f46cbb1560823976a8f342a37dc494a",
                ),
            ),
            (
                "analysis_revision",
                text(
                    field_type(ty, "analysis_revision"),
                    "6f49fd7b10ff09b699b33b05cb485f0b261e1702e261935d1be67edc6a7ddee9",
                ),
            ),
        ],
    )
}
fn pos(
    f: &Fixture,
    sentence: &Sentence,
    basis: &StructuredInfoValue,
    count_: u64,
) -> StructuredInfoValue {
    let ty = f.ty("LanguageParserPosEvidence");
    let collection = field_type(ty, "pos");
    let StructuredInfoTypeShape::Collection { element, .. } = collection.shape() else {
        panic!("POS")
    };
    record(
        ty,
        vec![
            ("basis", basis.clone()),
            ("token_count", number(field_type(ty, "token_count"), count_)),
            (
                "pos",
                StructuredInfoValue::collection(
                    collection.clone(),
                    (0..4)
                        .map(|i| number(element, *sentence.pos.get(i).unwrap_or(&16)))
                        .collect(),
                )
                .unwrap(),
            ),
        ],
    )
}
fn complete_value(
    evaluator: &mut PreparedPortableExpressionEvaluator,
    state: &StructuredInfoValue,
) -> bool {
    let value = evaluator
        .evaluate(&state.canonical_bytes().unwrap())
        .unwrap();
    assert!(value == [0] || value == [1]);
    value == [1]
}
#[test]
fn representative_native_mapping_exposes_greedy_dead_end() {
    evaluate_native_ewt(Some(1));
}
#[test]
#[ignore = "explicit corpus evidence; greedy diagnostic is not beam/revision acceptance"]
fn pinned_learned_artifact_scores_native_streaming_source_graphs_on_heldout_ewt() {
    evaluate_native_ewt(None);
}
fn timed<T>(timing: &mut (u128, u128), operation: impl FnOnce() -> T) -> T {
    let started = std::time::Instant::now();
    let result = operation();
    let nanos = started.elapsed().as_nanos();
    timing.0 += nanos;
    timing.1 = timing.1.max(nanos);
    result
}
fn evaluate_native_ewt(diagnostic_limit: Option<usize>) {
    let preparation_started = std::time::Instant::now();
    let mut sentences = include_str!("../training/ewt_four_token/test_annotations.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<Sentence>(line).unwrap())
        .collect::<Vec<_>>();
    if let Some(limit) = diagnostic_limit {
        sentences.truncate(limit);
    }
    let mut f = Fixture::new();
    let mut scorer = scorer_model::Scorer::new();
    let features_blueprint =
        parser_kernel::Blueprint::prepare(source(), "language-parser-model-features");
    let transitions_blueprint =
        parser_kernel::Blueprint::prepare(source(), "language-parser-transition");
    let proposal_blueprint =
        parser_kernel::Blueprint::prepare(source(), "language-parser-score-proposal");
    let mut growth = f.prepare("language-parser-grow-buffer");
    let mut complete = f.prepare("language-parser-decode-complete");
    let source_preparation_ms = preparation_started.elapsed().as_millis();
    let mut plan_timing = (0, 0);
    let mut preparation_phases = [0u128; 4];
    let mut feature_timing = (0, 0);
    let mut proposal_timing = (0, 0);
    let mut transition_timing = (0, 0);
    let mut feature_sequence = 0;
    let mut transition_sequence = 0;
    let mut uas = 0;
    let mut las = 0;
    let mut base_las = 0;
    let mut tokens = 0;
    let mut finished = 0;
    let mut voc_gold = 0;
    let mut voc_pred = 0;
    let mut voc_correct = 0;
    let mut delay_sum = 0;
    let mut delay_max = 0;
    let mut assigned = 0;
    let mut revisions = 0;
    let mut scorer_nanos = 0;
    let started = std::time::Instant::now();
    for (sentence_index, sentence) in sentences.iter().enumerate() {
        let plan_started = std::time::Instant::now();
        let mut features = features_blueprint.realize(sentence_index);
        features.kernel.start().unwrap();
        let mut transitions = transitions_blueprint.realize(sentence_index);
        transitions.kernel.start().unwrap();
        let mut proposals = proposal_blueprint.realize(sentence_index);
        proposals.kernel.start().unwrap();
        for (i, total) in preparation_phases.iter_mut().enumerate() {
            *total += features.preparation_nanoseconds[i]
                + proposals.preparation_nanoseconds[i]
                + transitions.preparation_nanoseconds[i];
        }
        let plan_ns = plan_started.elapsed().as_nanos();
        plan_timing.0 += plan_ns;
        plan_timing.1 = plan_timing.1.max(plan_ns);
        let mut sentence_proposal_sequence = 0;
        let mut sentence_feature_sequence = 0;
        let mut sentence_transition_sequence = 0;
        let basis = basis(&f, &sentence.id);
        let mut state = replace(&f.initial(1), "basis", basis.clone());
        let mut first_assignment = [None; 4];
        let mut exhausted = false;
        for available in 1..=sentence.pos.len() {
            let ty = f.ty("LanguageParserBufferGrowth");
            let native_state = retype(field_type(ty, "state"), &state);
            let request = record(
                ty,
                vec![
                    ("state", native_state),
                    (
                        "available",
                        number(field_type(ty, "available"), available as u64),
                    ),
                ],
            );
            let admitted = admitted::LanguageParserBufferGrowth::from_structured(request).unwrap();
            state = evaluate(&mut growth, &admitted.into_structured().unwrap());
            for _ in 0..12 {
                if count(field(&state, "unread")) == available as u64
                    && available < sentence.pos.len()
                {
                    break;
                }
                if complete_value(&mut complete, &retype(f.ty("LanguageParserState"), &state)) {
                    break;
                }
                let ty = f.ty("LanguageParserScorerQuery");
                let query = record(
                    ty,
                    vec![
                        ("state", retype(field_type(ty, "state"), &state)),
                        ("lexical", pos(&f, sentence, &basis, available as u64)),
                    ],
                );
                let query = admitted::LanguageParserScorerQuery::from_structured(query)
                    .unwrap()
                    .into_structured()
                    .unwrap();
                let extracted = timed(&mut feature_timing, || {
                    features.transact(sentence_feature_sequence, &query)
                });
                feature_sequence += 1;
                sentence_feature_sequence += 1;
                let indices = (0..7)
                    .map(|i| count(index(field(&extracted, "indices"), i)))
                    .collect::<Vec<_>>();
                let compute_started = std::time::Instant::now();
                let (scores, evidence) =
                    scorer.score(&indices, extracted.semantic_digest().unwrap());
                scorer_nanos += compute_started.elapsed().as_nanos();
                assert_eq!(
                    evidence.artifact_identity,
                    scorer.artifact.content_identity()
                );
                assert_eq!(
                    evidence.signature_identity,
                    scorer.signature.semantic_digest().unwrap()
                );
                assert_eq!(
                    evidence.input_identities,
                    [extracted.semantic_digest().unwrap()]
                );
                let mut ranking = (0..scores.len()).collect::<Vec<_>>();
                ranking.sort_by(|a, b| scores[*b].cmp(&scores[*a]).then(a.cmp(b)));
                if sentence_index == 0 {
                    eprintln!(
                        "FIRST_ROW features={indices:?} top={:?}",
                        ranking
                            .iter()
                            .take(3)
                            .map(|i| (*i, scores[*i]))
                            .collect::<Vec<_>>()
                    );
                }
                let mut accepted_state = None;
                for class in ranking {
                    let ty = f.ty("LanguageParserScoredClass");
                    let query = record(
                        ty,
                        vec![
                            ("basis", basis.clone()),
                            ("state", state.clone()),
                            ("class", number(field_type(ty, "class"), class as u64)),
                            (
                                "score",
                                StructuredInfoValue::leaf(
                                    field_type(ty, "score").clone(),
                                    scores[class].to_le_bytes().to_vec(),
                                )
                                .unwrap(),
                            ),
                            ("default_relation", f.relation("dep")),
                        ],
                    );
                    let query = admitted::LanguageParserScoredClass::from_structured(query)
                        .unwrap()
                        .into_structured()
                        .unwrap();
                    let proposed = timed(&mut proposal_timing, || {
                        proposals.transact(sentence_proposal_sequence, &query)
                    });
                    sentence_proposal_sequence += 1;
                    let result = timed(&mut transition_timing, || {
                        transitions
                            .transact(sentence_transition_sequence, field(&proposed, "request"))
                    });
                    transition_sequence += 1;
                    sentence_transition_sequence += 1;
                    if sentence_index == 0 && sentence_proposal_sequence < 8 {
                        eprintln!(
                            "FIRST_ROW class={class} action={} relation={} accepted={}",
                            tag(field(field(&proposed, "request"), "action")),
                            tag(field(
                                field(field(&proposed, "request"), "relation"),
                                "base"
                            )),
                            accepted(&result)
                        );
                    }
                    if accepted(&result) {
                        accepted_state = Some(field(&result, "state").clone());
                        break;
                    }
                    assert_eq!(field(&result, "state"), &state);
                }
                let Some(next) = accepted_state else {
                    exhausted = true;
                    break;
                };
                assert!(f.native_ok(&next));
                for dependent in 0..available {
                    let old_head = count(index(field(&state, "heads"), dependent));
                    if old_head < 5
                        && (old_head != count(index(field(&next, "heads"), dependent))
                            || field(&state, &format!("relation{dependent}"))
                                != field(&next, &format!("relation{dependent}")))
                    {
                        revisions += 1;
                    }
                }
                for (dependent, first) in first_assignment
                    .iter_mut()
                    .enumerate()
                    .take(sentence.pos.len())
                {
                    if first.is_none() && count(index(field(&next, "heads"), dependent)) < 5 {
                        *first = Some(available as u64);
                    }
                }
                state = next;
            }
            if exhausted {
                break;
            }
        }
        finished += usize::from(complete_value(
            &mut complete,
            &retype(f.ty("LanguageParserState"), &state),
        ));
        for (dependent, gold_head) in sentence.heads.iter().enumerate() {
            tokens += 1;
            let head = count(index(field(&state, "heads"), dependent));
            let relation = field(&state, &format!("relation{dependent}"));
            let predicted = tag(field(relation, "base"));
            let gold = &sentence.relations[dependent];
            let correct_head = head == *gold_head;
            uas += usize::from(correct_head);
            las += usize::from(correct_head && predicted == gold);
            base_las += usize::from(correct_head && predicted == gold.split(':').next().unwrap());
            voc_gold += usize::from(gold == "vocative");
            voc_pred += usize::from(head < 5 && predicted == "vocative");
            voc_correct +=
                usize::from(correct_head && gold == "vocative" && predicted == "vocative");
            if let Some(when) = first_assignment[dependent] {
                let delay = when - (dependent as u64 + 1);
                delay_sum += delay;
                delay_max = delay_max.max(delay);
                assigned += 1;
            }
        }
        if sentence_index % 50 == 0 {
            eprintln!(
                "native heldout sentences={} tokens={} transitions={}",
                sentence_index + 1,
                tokens,
                transition_sequence
            );
        }
    }
    let process_memory = std::fs::read_to_string("/proc/self/status")
        .ok()
        .map(|status| {
            status
                .lines()
                .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
                .collect::<Vec<_>>()
                .join("; ")
        });
    let metrics = serde_json::json!({"preparation_phases_nanoseconds_planning_lowering_owners_kernel":preparation_phases,"host_process_memory_status":process_memory,"source_preparation_milliseconds":source_preparation_ms,"plan_realization_nanoseconds_sum":plan_timing.0,"plan_realization_nanoseconds_maximum":plan_timing.1,"feature_graph_nanoseconds_sum":feature_timing.0,"feature_graph_nanoseconds_maximum":feature_timing.1,"proposal_graph_nanoseconds_sum":proposal_timing.0,"proposal_graph_nanoseconds_maximum":proposal_timing.1,"transition_graph_nanoseconds_sum":transition_timing.0,"transition_graph_nanoseconds_maximum":transition_timing.1,"diagnostic_limit":diagnostic_limit,"proof":"ordinary Plan/Play source features/transitions + exact hosted integer model", "profile":"four-token greedy beam width1 gold-UPOS; immutable-source availability streamed", "sentences":sentences.len(),"tokens":tokens,"complete_sentences":finished,"uas_correct":uas,"las_correct":las,"universal_base_las_correct":base_las,"vocative_gold":voc_gold,"vocative_predicted":voc_pred,"vocative_correct_head_and_label":voc_correct,"assigned_edges":assigned,"assignment_delay_tokens_sum":delay_sum,"assignment_delay_tokens_maximum":delay_max,"source_feature_requests":feature_sequence,"source_transition_requests":transition_sequence,"native_elapsed_ms":started.elapsed().as_millis(),"model_bytes":scorer_model::BYTES.len(),"compute_work_units_per_score":scorer.adapter.work_units(),"model_content_identity":scorer.artifact.content_identity().iter().map(|b|format!("{b:02x}")).collect::<String>(),"observed_edge_revision_count":revisions,"committed_edges_evaluated":0,"committed_later_contradiction_percentage":serde_json::Value::Null,"scorer_compute_nanoseconds":scorer_nanos,"numeric_working_memory_bound_bytes":1048576,"limitation":"arc assignments in this profile are monotonic; no later text correction tested, singleton agreement is not beam4 stabilization evidence"});
    eprintln!("NATIVE_LEARNED_METRICS {}", metrics);
    if let Ok(path) = std::env::var("CONDUIT_PARSER_NATIVE_METRICS_OUTPUT") {
        std::fs::write(path, serde_json::to_string_pretty(&metrics).unwrap()).unwrap();
    }
    if diagnostic_limit.is_none() {
        assert_eq!(sentences.len(), 533);
        assert_eq!(tokens, 1249);
        assert_eq!(voc_gold, 7);
    }
    assert!(tokens > 0);
    if diagnostic_limit == Some(1) {
        assert_eq!((tokens, uas, finished, transition_sequence), (2, 1, 0, 78));
    }
}

#[test]
fn exact_model_and_feature_admission_refuse_corruption_and_foreign_basis() {
    use conduit_ai::integer_categorical::IntegerCategoricalModel;
    let mut f = Fixture::new();
    let scorer = scorer_model::Scorer::new();
    let model =
        IntegerCategoricalModel::prepare(&scorer.artifact, &scorer.signature, scorer_model::BYTES)
            .unwrap();
    let weights = model.weights_tensor().unwrap();
    weights.validate().unwrap();
    assert_eq!(weights.dimensions.as_slice(), [76, 374]);
    assert_eq!(weights.element, conduit_data::TensorElement::I16);
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../training/ewt_four_token/manifest.json")).unwrap();
    let hex = |digest: [u8; 32]| {
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(
        hex(weights.content_digest),
        manifest["weights_tensor_content_identity"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        hex(model.identity()),
        manifest["model_content_identity"].as_str().unwrap()
    );
    let mut corrupt = scorer_model::BYTES.to_vec();
    corrupt[24] ^= 1;
    assert!(
        IntegerCategoricalModel::prepare(&scorer.artifact, &scorer.signature, &corrupt).is_err()
    );
    let mut wrong = scorer.artifact.clone();
    wrong.signature_identity[0] ^= 1;
    assert!(
        IntegerCategoricalModel::prepare(&wrong, &scorer.signature, scorer_model::BYTES).is_err()
    );
    let sentence = Sentence {
        id: "source-owned-admission".into(),
        pos: vec![15],
        heads: vec![4],
        relations: vec!["root".into()],
    };
    let state = f.initial(1);
    let ty = f.ty("LanguageParserScorerQuery");
    let query = record(
        ty,
        vec![
            ("state", retype(field_type(ty, "state"), &state)),
            (
                "lexical",
                pos(&f, &sentence, &f.basis("analysis/foreign"), 1),
            ),
        ],
    );
    assert!(admitted::LanguageParserScorerQuery::from_structured(query).is_err());
}
