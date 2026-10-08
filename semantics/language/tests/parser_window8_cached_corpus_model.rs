//! Actual cached-bank final clause replay; optional external references affect
//! evaluation only. Independent lexical consensus is checked separately.
#![cfg(feature = "parser-model-selection")]
extern crate alloc;
#[path = "common/dependency_metrics.rs"]
mod dependency_metrics;
#[path = "common/window8_fact_replay.rs"]
mod facts;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/window8_cached_model.rs"]
mod model;
#[path = "../src/parser_window8_program_bank.rs"]
#[allow(dead_code)]
mod owned_bank;
#[path = "common/parser_planned_runtime.rs"]
#[allow(dead_code)] // The shared v2 wrapper is unused by this distinct Source entry.
mod planned;
#[path = "common/parser_model_resource.rs"]
mod resource;
use conduit_core::*;
use conduit_language::{
    parser_model_selection::*,
    parser_window8::lexical::*,
    *,
};
use conduit_plot::rust_binding::NativeRustBinding;
use serde_json::{json, Value};
use std::path::PathBuf;
fn bytes_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
#[test]
#[ignore = "explicit long native window8 teaching decode; no heldout accuracy claim"]
fn actual_cached_window8_reviewed_clause_decode() {
    let directory =
        PathBuf::from(std::env::var("WINDOW8_MODEL_DIR").expect("explicit exact model directory"));
    let weights = std::fs::read(directory.join("ewt_window8.i16")).unwrap();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    let profile_bytes = std::fs::read(directory.join("lexical_profile.json")).unwrap();
    assert_eq!(weights.len(), 64620);
    assert_eq!(
        manifest["feature_class_contract_identity"],
        model::hex(model::feature_contract())
    );
    assert_eq!(
        manifest["model_content_identity"],
        model::hex(conduit_ai::model_content_digest(&weights))
    );
    let teaching_override = std::env::var_os("WINDOW8_TEACHING_ROWS");
    let teaching_bytes = teaching_override.as_ref().map_or_else(
        || include_bytes!("../training/ewt_joint_v3_window8/reviewed_teaching.json").to_vec(),
        |path| std::fs::read(path).expect("exact candidate teaching input"),
    );
    if let Some(identity) = manifest["teaching_content_identity"].as_str() {
        assert_eq!(
            identity,
            model::hex(semantic_digest(
                "language/parser-teaching@1",
                &teaching_bytes
            )),
            "teaching input differs from the trained model manifest"
        );
    } else {
        assert!(
            teaching_override.is_none(),
            "candidate teaching requires a pinned content identity"
        );
        assert_eq!(
            model::hex(semantic_digest(
                "language/parser-teaching@1",
                &teaching_bytes
            )),
            "f62146aa20b5e2360360c3d9597b99e14e56e8e0894429f0f148435c5eb31bd0",
            "embedded teaching bytes differ from the original legacy input"
        );
        assert_eq!(
            manifest["reviewed_teaching_sha256"],
            "562a8b8eeb72dd5f2549849d707975a954992f3c4f86df8a9f596383f56eaa5f",
            "legacy model must retain the original embedded teaching input"
        );
    }
    let teaching: Value = serde_json::from_slice(&teaching_bytes).unwrap();
    let external = std::env::var_os("WINDOW8_EVALUATION_ROWS");
    let evaluation = external.is_some();
    let rows: Value = external.map_or_else(
        || teaching.clone(),
        |path| serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap(),
    );
    // Separate evidence prevents an evaluation from overwriting TRAIN receipts.
    let output_directory = if evaluation {
        PathBuf::from(
            std::env::var_os("WINDOW8_EVALUATION_OUTPUT")
                .expect("separate evaluation output directory"),
        )
    } else {
        directory.clone()
    };
    std::fs::create_dir_all(&output_directory).unwrap();
    if evaluation {
        assert_ne!(
            output_directory.canonicalize().unwrap(),
            directory.canonicalize().unwrap(),
            "evaluation must not overwrite model-directory TRAIN evidence"
        );
    }
    assert!(!rows.as_array().unwrap().is_empty());
    let mut identities = std::collections::BTreeSet::new();
    for row in rows.as_array().unwrap() {
        assert!(identities.insert(row["id"].as_str().unwrap()));
        let count = row["forms"].as_array().unwrap().len();
        assert!((1..=8).contains(&count));
        for field in ["pos", "heads", "relations"] {
            assert_eq!(row[field].as_array().unwrap().len(), count);
        }
        if evaluation {
            assert!(
                !teaching
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|train| train["text"] == row["text"]),
                "external evaluation text overlaps reviewed teaching data"
            );
        }
    }
    // No whole-training disjointness claim: that requires the model's complete
    // supervision/membership manifest, beyond this teaching-overlap check.
    let lexical_profile = model::lexical(&profile_bytes);
    let scorer = resource::categorical(weights, model::signature(), 1);
    let (definition, contracts) = model::declaration(&scorer, &lexical_profile);
    let selected = PreparedParserModelSelection::prepare_declared(
        scorer.clone(),
        definition,
        &lexical_profile,
        &contracts,
    )
    .unwrap();
    eprintln!("window8 cached corpus: numeric Source preparation start");
    let mut execution = planned::prepare_source_with_inference_budget(
        scorer.clone(),
        model::source(&scorer),
        "window8-learned-model",
        4096,
    );
    eprintln!("window8 cached corpus: numeric Source preparation complete");
    let prepared_native = std::env::var_os("WINDOW8_PREPARED_NATIVE")
        .map(|value| {
            assert_eq!(value, "1", "explicit prepared Native opt-in must be 1");
            true
        })
        .unwrap_or(false);
    let bank = if prepared_native {
        owned_bank::Window8ProgramBank::prepare_native(
            conduit_plot::rust_binding::PreparedNativeFamilyLimits {
                maximum_types: 64,
                maximum_laws_per_type: 64,
                maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
                maximum_retained_bytes: 256 * 1024 * 1024,
                maximum_preparation_peak_bytes: 512 * 1024 * 1024,
                maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
            },
        )
        .expect("complete exact Native output family must prepare before replay")
    } else {
        owned_bank::Window8ProgramBank::prepare().unwrap()
    };
    let native_admission_receipt = bank.native_storage_receipt().map(|receipt| json!({
        "types": receipt.types,
        "retained_heap_bytes_bound": receipt.retained_heap_bytes_bound,
        "preparation_peak_heap_bytes_bound": receipt.preparation_peak_heap_bytes_bound,
        "conversion_requested_bytes_bound": receipt.conversion_requested_bytes_bound,
        "scope": "Native bank output admission only; excludes Reference Source evaluation, ordinary input encoding, model execution, retained outputs and queues"
    }));
    eprintln!("window8 Native admission receipt: {native_admission_receipt:?}");
    eprintln!("window8 cached corpus: fact Source preparation start");
    let fact_schema = facts::FactSchema::prepare();
    eprintln!("window8 cached corpus: fact Source preparation complete");
    let mut vocative_edges = dependency_metrics::VocativeEdges::default();
    let mut tokens = 0usize;
    let mut correct_heads = 0usize;
    let mut correct_base_labels = 0usize;
    let mut correct_pos = 0usize;
    let default = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let classes = (0..76)
        .map(|code| bank.class(code, &default).unwrap())
        .collect::<Vec<_>>();
    let mut invocations = 0;
    let mut receipts = Vec::new();
    let mut correct = 0;
    let started = std::time::Instant::now();
    for row in rows.as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let source = LanguageTextRevision::new(
            LanguageTextFinality::Final,
            LanguageText::new(
                LanguageTextId::new(format!("window8/{id}")).unwrap(),
                LanguageId::new("language/en".into()).unwrap(),
                LanguageTextRevisionId::new(format!("window8/{id}/r0")).unwrap(),
                row["text"].as_str().unwrap().into(),
            )
            .unwrap(),
            None,
            model::provenance(),
            0,
            None,
        )
        .unwrap();
        let tape = conduit_language::lexical::prepare_lexical_tape(&source, &lexical_profile, None)
            .unwrap();
        assert_eq!(
            tape.tape()
                .tokens()
                .as_slice()
                .iter()
                .map(|token| token.surface().as_str())
                .collect::<Vec<_>>(),
            row["forms"]
                .as_array()
                .unwrap()
                .iter()
                .map(|form| form.as_str().unwrap())
                .collect::<Vec<_>>(),
            "reference token occurrences must match actual lexical reconstruction"
        );
        let lexical = prepare_window8_lexical(&tape).unwrap();
        let mut analysis_material = tape.tape().clone().encode().unwrap();
        analysis_material.extend_from_slice(&selected.compatibility().model_content);
        analysis_material.extend_from_slice(&selected.compatibility().signature);
        analysis_material.extend_from_slice(&contracts.feature_contract);
        analysis_material.extend_from_slice(&contracts.availability_contract);
        analysis_material.extend_from_slice(&contracts.action_contract);
        analysis_material.extend_from_slice(&contracts.numeric_indices_contract);
        analysis_material.extend_from_slice(&contracts.numeric_scores_contract);
        analysis_material.extend_from_slice(&contracts.joint_choice_contract);
        let analysis = model::hex(semantic_digest(
            "language/parser-window8-analysis@3",
            &analysis_material,
        ));
        let basis = LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new(analysis).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap();
        eprintln!("window8 cached corpus {id}: source/lexical admitted, initialize");
        let initial = bank
            .initialize(
                &LanguageParserWindow8Begin::new(
                    basis.clone(),
                    default.clone(),
                    *lexical.lexical().token_count(),
                )
                .unwrap(),
            )
            .unwrap();
        let empty = |identity, active| {
            LanguageParserWindow8RawHypothesis::new(
                active,
                [0; 8],
                identity,
                0,
                0,
                initial.state().clone(),
            )
            .unwrap()
        };
        let mut beam = LanguageParserWindow8RawBeam::new(
            empty(0, true),
            empty(1, false),
            empty(2, false),
            empty(3, false),
        )
        .unwrap();
        let mut identity = 4;
        let mut epochs = Vec::new();
        for epoch in 0..32 {
            eprintln!("window8 cached corpus {id}: epoch {epoch} start");
            let mut next = LanguageParserWindow8RawBeam::new(
                empty(0, false),
                empty(1, false),
                empty(2, false),
                empty(3, false),
            )
            .unwrap();
            let candidates = [
                beam.candidate0(),
                beam.candidate1(),
                beam.candidate2(),
                beam.candidate3(),
            ];
            for prior in candidates
                .into_iter()
                .filter(|candidate| *candidate.active())
            {
                let state = bank.admit_state(prior.state()).unwrap();
                if bank.complete(&state).unwrap() {
                    next = bank.merge(next, prior.clone()).unwrap();
                    continue;
                }
                let context = bank.context(&state, &basis).unwrap();
                let unread = *prior.state().unread() as usize;
                let choose = unread < *lexical.lexical().token_count() as usize
                    && *prior.selected() == unread as u64;
                let alternatives = if choose {
                    *lexical.projection().tokens()[unread].count()
                } else {
                    1
                };
                for choice in 0..alternatives {
                    let mut choices = *prior.choices();
                    if choose {
                        choices[unread] = choice;
                    }
                    let features = bank.features(&state, &lexical, &basis, choices).unwrap();
                    let choice_query = LanguageParserWindow8ChoiceQuery::new(
                        features.query().clone(),
                        *prior.choices(),
                        *prior.selected(),
                    )
                    .unwrap();
                    let frontier = bank.choice_frontier(choice_query).unwrap();
                    eprintln!(
                        "window8 cached corpus {id}: inference {invocations} epoch {epoch} start"
                    );
                    let scores = execution.infer(
                        invocations,
                        &features.features().raw().clone().into_structured().unwrap(),
                    );
                    invocations += 1;
                    let StructuredInfoValueShape::Collection(scores) = scores.shape() else {
                        panic!("bare numerical scores")
                    };
                    for (class, value) in classes.iter().zip(scores) {
                        let proposal = context.propose(class).unwrap();
                        if !proposal.proposal().accepted() {
                            continue;
                        }
                        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                            panic!("I64 scalar")
                        };
                        let score = i64::from_le_bytes(bytes.try_into().unwrap());
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
                                .unwrap(),
                            )
                            .unwrap();
                        identity += 1;
                        next = bank.merge(next, raw).unwrap();
                    }
                }
            }
            beam = next;
            eprintln!("window8 cached corpus {id}: epoch {epoch} candidates retained");
            let mut proofs = Vec::new();
            for candidate in [
                beam.candidate0(),
                beam.candidate1(),
                beam.candidate2(),
                beam.candidate3(),
            ]
            .into_iter()
            .filter(|candidate| *candidate.active())
            {
                let proof = bank.admit_state(candidate.state()).unwrap();
                proofs.push(bytes_hex(&proof.proof().clone().encode().unwrap()));
            }
            epochs.push(json!({"epoch":epoch,"completed_model_invocations":invocations,"beam_bytes":bytes_hex(&beam.clone().encode().unwrap()),"state_proof_bytes":proofs}));
            if [
                beam.candidate0(),
                beam.candidate1(),
                beam.candidate2(),
                beam.candidate3(),
            ]
            .into_iter()
            .filter(|candidate| *candidate.active())
            .all(|candidate| {
                bank.complete(&bank.admit_state(candidate.state()).unwrap())
                    .unwrap()
            }) {
                break;
            }
        }
        assert!(*beam.candidate0().active(), "{id}");
        let preferred = beam.candidate0();
        let state = bank.admit_state(preferred.state()).unwrap();
        let complete = bank.complete(&state).unwrap();
        let proofs = [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ]
        .map(|candidate| bank.admit_state(candidate.state()).unwrap().proof().clone());
        let admitted_fact = fact_schema.lexical_fact(lexical.lexical(), &basis, &beam, &proofs, 1);
        let (fact_bytes, fact_refusal) = match admitted_fact {
            Ok(value) => (Some(bytes_hex(&value.canonical_bytes().unwrap())), None),
            Err(refusal) => (None, Some(refusal)),
        };
        eprintln!(
            "window8 cached corpus {id}: independent lexical fact accepted={}",
            fact_bytes.is_some()
        );
        let count = *lexical.lexical().token_count() as usize;
        let pos = (0..count)
            .map(|ordinal| {
                lexical.projection().tokens()[ordinal].codes()
                    [preferred.choices()[ordinal] as usize]
            })
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
        let predicted=(0..count).map(|ordinal|json!({"dependent":ordinal,"head":state.state().heads()[ordinal],"base":format!("{:?}",relations[ordinal].base()).to_lowercase()})).collect::<Vec<_>>();
        let matches = complete
            && pos
                .iter()
                .enumerate()
                .all(|(i, value)| row["pos"][i].as_u64() == Some(*value))
            && predicted.iter().enumerate().all(|(i, value)| {
                value["head"] == row["heads"][i]
                    && value["base"].as_str()
                        == row["relations"][i].as_str().unwrap().split(':').next()
            });
        correct += usize::from(matches);
        tokens += count;
        for ordinal in 0..count {
            let head_matches = predicted[ordinal]["head"] == row["heads"][ordinal];
            vocative_edges.observe(
                predicted[ordinal]["base"].as_str() == Some("vocative"),
                row["relations"][ordinal]
                    .as_str()
                    .unwrap()
                    .split(':')
                    .next()
                    == Some("vocative"),
                head_matches,
            );
            correct_heads += usize::from(head_matches);
            correct_base_labels += usize::from(
                head_matches
                    && predicted[ordinal]["base"].as_str()
                        == row["relations"][ordinal]
                            .as_str()
                            .unwrap()
                            .split(':')
                            .next(),
            );
            correct_pos += usize::from(row["pos"][ordinal].as_u64() == Some(pos[ordinal]));
        }
        eprintln!(
            "actual cached corpus window8 {id}: complete={complete}, exact_base_graph={matches}"
        );
        receipts.push(json!({"id":id,"text":row["text"],"text_identity":source.material().identity().get(),"source_revision":source.material().revision().get(),"analysis_revision":basis.analysis_revision().get(),"source_material_bytes":bytes_hex(&source.clone().encode().unwrap()),"lexical_tape_bytes":bytes_hex(&tape.tape().clone().encode().unwrap()),"basis":bytes_hex(&basis.clone().encode().unwrap()),"state_proof_bytes":bytes_hex(&state.proof().clone().encode().unwrap()),"choices":preferred.choices(),"pos":pos,"predicted":predicted,"complete":complete,"lexical_fact_bytes":fact_bytes,"lexical_fact_refusal":fact_refusal,"epochs":epochs,"root_sentinel":8,"reviewed_train_teaching_demonstration":!evaluation,"external_evaluation_references":evaluation,"model_content":model::hex(selected.compatibility().model_content),"feature_contract":model::hex(contracts.feature_contract),"choice_contract":model::hex(contracts.joint_choice_contract)}));
        std::fs::write(
            output_directory.join("native_cached_clause_rows.json"),
            serde_json::to_vec_pretty(&receipts).unwrap(),
        )
        .unwrap();
    }
    let result = json!({"native_admission_receipt":native_admission_receipt,"prepared_native_output_admission":prepared_native,"receipts":receipts,"exact_base_graphs":correct,"exact_reference_teaching_graphs":if evaluation {None} else {Some(correct)},"examples":rows.as_array().unwrap().len(),"actual_model_invocations":invocations,"admitted_inference_bound":4096,"elapsed_nanos":started.elapsed().as_nanos().to_string(),"heldout_accuracy_claim":false,"stable_or_played_fact_claim":false,"external_evaluation_references":evaluation,"training_membership_disjointness_verified":false,"metric_token_scope":"all supplied tokens including punctuation; universal base labels only, subtypes excluded","tokens":tokens,"correct_heads":correct_heads,"correct_base_labels":correct_base_labels,"correct_pos":correct_pos,"uas":correct_heads as f64 / tokens as f64,"base_las":correct_base_labels as f64 / tokens as f64,"pos_accuracy":correct_pos as f64 / tokens as f64});
    let mut result = result;
    result["vocative_edges"] = json!({
        "scope": "exact dependent occurrence, governor and universal base relation; wrong governor counts as both false positive and false negative",
        "true_positive": vocative_edges.true_positive,
        "false_positive": vocative_edges.false_positive,
        "false_negative": vocative_edges.false_negative,
        "precision": vocative_edges.precision(),
        "recall": vocative_edges.recall(),
        "undefined_denominator": "null; no perfect-score substitution",
    });
    std::fs::write(
        output_directory.join("native_cached_clause_decode.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    if !evaluation {
        assert_eq!(correct, rows.as_array().unwrap().len());
    }
}
