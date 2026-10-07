//! Fresh actual Partial acquisition with Source wait authority and flushed native observer events.
#![cfg(feature = "parser-model-selection")]
extern crate alloc;
#[path = "common/window8_fact_replay.rs"]
mod facts;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/window8_cached_model.rs"]
mod model;
#[path = "common/window8_observer.rs"]
mod observer;
#[path = "../src/parser_window8_program_bank.rs"]
#[allow(dead_code)]
mod owned_bank;
#[path = "common/parser_planned_runtime.rs"]
#[allow(dead_code)] // The shared v2 wrapper is unused by this distinct Source entry.
mod planned;
#[path = "common/parser_model_resource.rs"]
mod resource;
#[path = "common/window8_wait_replay.rs"]
mod wait;
use conduit_core::*;
use conduit_language::{
    parser_model_selection::*,
    parser_window8::{lexical, lexical::*, *},
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
fn actual_partial_available_context_emits_native_boundaries() {
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
    eprintln!("window8 cached partial: numeric Source preparation start");
    let mut execution = planned::prepare_source_with_inference_budget(
        scorer.clone(),
        model::source(&scorer),
        "window8-learned-model",
        2048,
    );
    eprintln!("window8 cached partial: numeric Source preparation complete");
    let bank = owned_bank::Window8ProgramBank::prepare().unwrap();
    eprintln!("window8 cached partial: fact Source preparation start");
    let fact_schema = facts::FactSchema::prepare();
    eprintln!("window8 cached partial: fact Source preparation complete");
    let wait_schema = wait::WaitSchema::prepare();
    let observer_path =
        PathBuf::from(std::env::var("WINDOW8_OBSERVER_PATH").expect("explicit fresh event path"));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(observer_path)
        .unwrap();
    let mut observer = observer::Observer::new(file);
    let rows = json!([
        {"id":"record-later-available","text":"I record the ","pos":[],"heads":[],"relations":[]},
        {"id":"record-noun-available","text":"I record the record ","pos":[],"heads":[],"relations":[]},
        {"id":"refuse-later-available","text":"They refuse the ","pos":[],"heads":[],"relations":[]},
        {"id":"refuse-noun-available","text":"They refuse the refuse ","pos":[],"heads":[],"relations":[]}
    ]);
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
    let started = std::time::Instant::now();
    for row in rows.as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let source = LanguageTextRevision::new(
            LanguageTextFinality::Partial,
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
            Some(row["text"].as_str().unwrap().chars().count() as u32),
        )
        .unwrap();
        let tape = conduit_language::lexical::prepare_lexical_tape(&source, &lexical_profile, None)
            .unwrap();
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
        observer.emit("availability", json!({"id":id,"text":row["text"],"source_material_bytes":bytes_hex(&source.clone().encode().unwrap()),"lexical_tape_bytes":bytes_hex(&tape.tape().clone().encode().unwrap()),"analysis_revision":basis.analysis_revision().get(),"model_content":model::hex(selected.compatibility().model_content),"model_invocations":invocations})).unwrap();
        eprintln!("window8 cached partial {id}: source/lexical admitted, initialize");
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
            eprintln!("window8 cached partial {id}: epoch {epoch} start");
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
                if wait_schema
                    .admit(state.proof(), lexical.lexical(), &basis)
                    .is_ok()
                {
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
                    observer.emit("model-inference-start", json!({"id":id,"epoch":epoch,"invocation":invocations,"feature_bytes":bytes_hex(&features.features().raw().clone().encode().unwrap())})).unwrap();
                    let scores = execution.infer(
                        invocations,
                        &features.features().raw().clone().into_structured().unwrap(),
                    );
                    invocations += 1;
                    observer.emit("model-inference-complete", json!({"id":id,"epoch":epoch,"completed_model_invocations":invocations,"scores_bytes":bytes_hex(&scores.canonical_bytes().unwrap())})).unwrap();
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
            eprintln!("window8 cached partial {id}: epoch {epoch} candidates retained");
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
            observer.emit("snapshot", json!({"id":id,"epoch":epoch,"text":row["text"],"source_revision":source.material().revision().get(),"analysis_revision":basis.analysis_revision().get(),"beam_bytes":bytes_hex(&beam.clone().encode().unwrap()),"state_proof_bytes":proofs,"model_invocations":invocations})).unwrap();
            epochs.push(json!({"epoch":epoch,"beam_bytes":bytes_hex(&beam.clone().encode().unwrap()),"state_proof_bytes":proofs}));
            if [
                beam.candidate0(),
                beam.candidate1(),
                beam.candidate2(),
                beam.candidate3(),
            ]
            .into_iter()
            .filter(|candidate| *candidate.active())
            .all(|candidate| {
                wait_schema
                    .admit(
                        bank.admit_state(candidate.state()).unwrap().proof(),
                        lexical.lexical(),
                        &basis,
                    )
                    .is_ok()
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
            "window8 cached partial {id}: independent lexical fact accepted={}",
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
        let waits = [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ]
        .into_iter()
        .filter(|candidate| *candidate.active())
        .map(|candidate| {
            wait_schema
                .admit(
                    bank.admit_state(candidate.state()).unwrap().proof(),
                    lexical.lexical(),
                    &basis,
                )
                .map(|value| bytes_hex(&value.canonical_bytes().unwrap()))
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
        observer.emit("lexical-fact", json!({"id":id,"fact_bytes":fact_bytes,"refusal":fact_refusal,"wait_bytes":waits,"model_invocations":invocations})).unwrap();
        receipts.push(json!({"id":id,"text":row["text"],"text_identity":source.material().identity().get(),"source_revision":source.material().revision().get(),"analysis_revision":basis.analysis_revision().get(),"source_material_bytes":bytes_hex(&source.clone().encode().unwrap()),"lexical_tape_bytes":bytes_hex(&tape.tape().clone().encode().unwrap()),"basis":bytes_hex(&basis.clone().encode().unwrap()),"state_proof_bytes":bytes_hex(&state.proof().clone().encode().unwrap()),"choices":preferred.choices(),"pos":pos,"predicted":predicted,"complete":complete,"lexical_fact_bytes":fact_bytes,"lexical_fact_refusal":fact_refusal,"epochs":epochs,"root_sentinel":8,"reviewed_prefix_demonstration":true,"model_content":model::hex(selected.compatibility().model_content),"feature_contract":model::hex(contracts.feature_contract),"choice_contract":model::hex(contracts.joint_choice_contract)}));
        std::fs::write(
            directory.join("native_available_partial_rows.json"),
            serde_json::to_vec_pretty(&receipts).unwrap(),
        )
        .unwrap();
    }
    let result = json!({"receipts":receipts,"examples":rows.as_array().unwrap().len(),"actual_model_invocations":invocations,"elapsed_nanos":started.elapsed().as_nanos().to_string(),"heldout_accuracy_claim":false,"stable_or_played_fact_claim":false});
    std::fs::write(
        directory.join("native_available_partial_decode.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}
