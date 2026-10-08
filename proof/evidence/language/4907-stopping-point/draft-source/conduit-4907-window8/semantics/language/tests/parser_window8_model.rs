//! Actual retained native model decoding. Reference rows only score predictions.
#![cfg(feature = "parser-model-selection")]
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/window8_model.rs"]
mod model;
#[path = "common/parser_planned_runtime.rs"]
#[allow(dead_code)] // The shared v2 wrapper is unused by this distinct Source entry.
mod planned;
#[path = "common/parser_model_resource.rs"]
mod resource;
use conduit_core::*;
use conduit_language::{
    parser_model_selection::*,
    parser_window8::{self, lexical::*},
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
fn actual_native_window8_teaching_model_retains_complete_graph_bases() {
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
    let mut execution = planned::prepare_source(
        scorer.clone(),
        model::source(&scorer),
        "window8-learned-model",
    );
    let rows: Value = serde_json::from_str(include_str!(
        "../training/ewt_joint_v3_window8/reviewed_teaching.json"
    ))
    .unwrap();
    let default = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let classes = (0..76)
        .map(|code| parser_window8::window8_class(code, &default).unwrap())
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
        let initial = parser_window8::initialize_window8(
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
                let state = parser_window8::prepare_window8_state(prior.state()).unwrap();
                if parser_window8::window8_complete(&state).unwrap() {
                    next = parser_window8::window8_merge(next, prior.clone()).unwrap();
                    continue;
                }
                let context = parser_window8::prepare_window8_context(&state, &basis).unwrap();
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
                    let features =
                        prepare_window8_features(&state, &lexical, &basis, choices).unwrap();
                    let choice_query = LanguageParserWindow8ChoiceQuery::new(
                        features.query().clone(),
                        *prior.choices(),
                        *prior.selected(),
                    )
                    .unwrap();
                    let frontier = parser_window8::window8_choice_frontier(choice_query).unwrap();
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
                        let raw = parser_window8::window8_score_advance(
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
                        next = parser_window8::window8_merge(next, raw).unwrap();
                    }
                }
            }
            beam = next;
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
                let proof = parser_window8::prepare_window8_state(candidate.state()).unwrap();
                proofs.push(bytes_hex(&proof.proof().clone().encode().unwrap()));
            }
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
                parser_window8::window8_complete(
                    &parser_window8::prepare_window8_state(candidate.state()).unwrap(),
                )
                .unwrap()
            }) {
                break;
            }
        }
        assert!(*beam.candidate0().active(), "{id}");
        let preferred = beam.candidate0();
        let state = parser_window8::prepare_window8_state(preferred.state()).unwrap();
        let complete = parser_window8::window8_complete(&state).unwrap();
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
        eprintln!("actual native window8 {id}: complete={complete}, exact_base_graph={matches}");
        receipts.push(json!({"id":id,"text":row["text"],"text_identity":source.material().identity().get(),"source_revision":source.material().revision().get(),"analysis_revision":basis.analysis_revision().get(),"source_material_bytes":bytes_hex(&source.clone().encode().unwrap()),"lexical_tape_bytes":bytes_hex(&tape.tape().clone().encode().unwrap()),"basis":bytes_hex(&basis.clone().encode().unwrap()),"state_proof_bytes":bytes_hex(&state.proof().clone().encode().unwrap()),"choices":preferred.choices(),"pos":pos,"predicted":predicted,"complete":complete,"epochs":epochs,"root_sentinel":8,"teaching_only":true,"model_content":model::hex(selected.compatibility().model_content),"feature_contract":model::hex(contracts.feature_contract),"choice_contract":model::hex(contracts.joint_choice_contract)}));
    }
    let result = json!({"receipts":receipts,"exact_teaching_graphs":correct,"examples":rows.as_array().unwrap().len(),"actual_model_invocations":invocations,"elapsed_nanos":started.elapsed().as_nanos().to_string(),"heldout_accuracy_claim":false,"stable_or_played_fact_claim":false});
    std::fs::write(
        directory.join("native_teaching_decode.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    assert_eq!(correct, rows.as_array().unwrap().len());
}
