//! Re-admit lexical facts from exact recorded actual partial beams; no model replay.
#![cfg(feature = "parser-model-selection")]
extern crate alloc;
#[path = "common/window8_fact_replay.rs"]
mod facts;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/window8_cached_model.rs"]
#[allow(dead_code)] // Exact declaration reused; numerical Source execution is absent.
mod model;
#[path = "../src/parser_window8_program_bank.rs"]
#[allow(dead_code)]
mod owned_bank;
#[path = "common/parser_model_resource.rs"]
mod resource;
use conduit_core::*;
use conduit_language::{
    parser_model_selection::*,
    parser_window8::{lexical, lexical::*, *},
    *,
};
use conduit_plot::rust_binding::NativeRustBinding;
use serde_json::{json, Value};
use std::path::PathBuf;
fn decode_hex(value: &Value) -> Vec<u8> {
    let text = value.as_str().unwrap();
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
#[test]
#[ignore = "explicit actual recorded receipt and immutable model directory"]
fn readmit_actual_partial_verb_and_noun_choices_without_new_model_execution() {
    let query_clock = std::time::Instant::now();
    let directory = PathBuf::from(std::env::var("WINDOW8_MODEL_DIR").unwrap());
    let receipt_path = PathBuf::from(std::env::var("WINDOW8_RECEIPTS").unwrap());
    let output = PathBuf::from(std::env::var("WINDOW8_FACT_OUTPUT").unwrap());
    let rows: Value = serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    let rows = match rows {
        Value::Array(rows) => rows,
        Value::Object(_) => vec![rows],
        _ => panic!("actual receipt object or array required"),
    };
    let weights = std::fs::read(directory.join("ewt_window8.i16")).unwrap();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["model_content_identity"],
        model::hex(conduit_ai::model_content_digest(&weights))
    );
    assert_eq!(
        manifest["feature_class_contract_identity"],
        model::hex(model::feature_contract())
    );
    let profile = model::lexical(&std::fs::read(directory.join("lexical_profile.json")).unwrap());
    let resource = resource::categorical(weights, model::signature(), 1);
    let (definition, contracts) = model::declaration(&resource, &profile);
    let selected =
        PreparedParserModelSelection::prepare_declared(resource, definition, &profile, &contracts)
            .unwrap();
    let bank = owned_bank::Window8ProgramBank::prepare().unwrap();
    let schema = facts::FactSchema::prepare();
    let mut results = Vec::new();
    for id in [
        "record-later-available",
        "record-noun-available",
        "refuse-later-available",
        "refuse-noun-available",
    ] {
        let Some(row) = rows.iter().find(|row| row["id"] == id) else {
            continue;
        };
        assert_eq!(
            row["model_content"],
            model::hex(selected.compatibility().model_content)
        );
        assert_eq!(
            row["feature_contract"],
            model::hex(contracts.feature_contract)
        );
        assert_eq!(
            row["choice_contract"],
            model::hex(contracts.joint_choice_contract)
        );
        let source =
            LanguageTextRevision::decode(&decode_hex(&row["source_material_bytes"])).unwrap();
        assert_eq!(*source.finality(), LanguageTextFinality::Partial);
        let prepared =
            conduit_language::lexical::prepare_lexical_tape(&source, &profile, None).unwrap();
        assert_eq!(
            prepared.tape().clone().encode().unwrap(),
            decode_hex(&row["lexical_tape_bytes"])
        );
        let lexical = prepare_window8_lexical(&prepared).unwrap();
        let basis = LanguageParserBasis::decode(&decode_hex(&row["basis"])).unwrap();
        let mut material = prepared.tape().clone().encode().unwrap();
        material.extend_from_slice(&selected.compatibility().model_content);
        material.extend_from_slice(&selected.compatibility().signature);
        for contract in [
            contracts.feature_contract,
            contracts.availability_contract,
            contracts.action_contract,
            contracts.numeric_indices_contract,
            contracts.numeric_scores_contract,
            contracts.joint_choice_contract,
        ] {
            material.extend_from_slice(&contract);
        }
        assert_eq!(
            basis.analysis_revision().get().as_str(),
            model::hex(semantic_digest(
                "language/parser-window8-analysis@3",
                &material
            ))
        );
        assert_eq!(basis.source_revision(), source.material().revision());
        assert_eq!(basis.text(), source.material().identity());
        let epoch = row["epochs"].as_array().unwrap().last().unwrap();
        let beam = LanguageParserWindow8RawBeam::decode(&decode_hex(&epoch["beam_bytes"])).unwrap();
        let proofs = [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ]
        .map(|candidate| bank.admit_state(candidate.state()).unwrap().proof().clone());
        // Preserve every actual retained hypothesis; no preferred-choice override.
        for dependent in [1, 3]
            .into_iter()
            .filter(|dependent| *dependent < *lexical.lexical().token_count())
        {
            let query_unix_nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_string();
            let query_elapsed_nanos = query_clock.elapsed().as_nanos().to_string();
            let (fact, refusal) =
                match schema.lexical_fact(lexical.lexical(), &basis, &beam, &proofs, dependent) {
                    Ok(value) => (Some(hex(&value.canonical_bytes().unwrap())), None),
                    Err(reason) => (None, Some(reason)),
                };
            if dependent == 1 && row["lexical_fact_bytes"].is_string() {
                assert_eq!(
                    fact.as_deref(),
                    row["lexical_fact_bytes"].as_str(),
                    "exact previously admitted fact retained"
                );
            }
            eprintln!(
                "actual recorded {id} dependent{dependent}: Source fact accepted={}",
                fact.is_some()
            );
            results.push(json!({"id":id,"dependent":dependent,"source_revision":source.material().revision().get(),"analysis_revision":basis.analysis_revision().get(),"recorded_beam_bytes":epoch["beam_bytes"],"lexical_fact_structured_bytes":fact,"refusal":refusal,"additional_model_invocations":0,"subsequent_query_unix_nanos":query_unix_nanos,"subsequent_query_elapsed_nanos":query_elapsed_nanos,"played_commitment_claim":false,"contemporaneous_producer_fact_claim":false}));
        }
    }
    assert!(
        !results.is_empty(),
        "at least one actual completed acquisition required"
    );
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    serde_json::to_writer_pretty(file,&json!({"schema":"language/window8-recorded-lexical-readmission@1","receipts":results,"actual_model_replayed":false,"heldout_accuracy_claim":false})).unwrap();
}
