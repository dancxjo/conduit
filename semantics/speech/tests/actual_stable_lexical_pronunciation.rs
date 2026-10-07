#![cfg(feature = "semantic-bindings")]
//! Re-admit a complete actual early lexical fact without inventing a dependency.
use conduit_core::StructuredInfoValue;
use conduit_language::{stable_lexical_selection::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::stable_lexical_pronunciation::*;
#[path = "common/pronunciation_fixture.rs"]
mod speech_fixture;

fn hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record required")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn readmit_checked(value: &StructuredInfoValue) -> LanguageParserWindow8CheckedHypothesis {
    let hypothesis =
        LanguageParserWindow8RawHypothesis::from_structured(field(value, "hypothesis").clone())
            .unwrap();
    let proof =
        LanguageParserWindow8StateProof::from_structured(field(value, "proof").clone()).unwrap();
    let admitted = LanguageParserWindow8CheckedHypothesis::new(hypothesis, proof).unwrap();
    assert_eq!(
        admitted.hypothesis().clone().into_structured().unwrap(),
        *field(value, "hypothesis")
    );
    assert_eq!(
        admitted.proof().clone().into_structured().unwrap(),
        *field(value, "proof")
    );
    admitted
}

#[test]
#[ignore = "requires actual available Partial receipt via CONDUIT_AVAILABLE_LEXICAL_RECEIPT"]
fn actual_partial_verb_fact_reaches_source_pronunciation_with_whole_custody() {
    check_actual(
        "CONDUIT_AVAILABLE_LEXICAL_RECEIPT",
        "I record the ",
        1,
        1,
        LanguageLexicalPos::Verb,
        1,
    );
}

#[test]
#[ignore = "requires actual recorded dep3 fact via CONDUIT_AVAILABLE_NOUN_LEXICAL_RECEIPT"]
fn actual_partial_noun_fact_reaches_source_pronunciation_with_whole_custody() {
    check_actual(
        "CONDUIT_AVAILABLE_NOUN_LEXICAL_RECEIPT",
        "I record the record ",
        3,
        0,
        LanguageLexicalPos::Noun,
        0,
    );
}

fn check_actual(
    receipt_variable: &str,
    text: &str,
    expected_dependent: u64,
    choice: u64,
    pos: LanguageLexicalPos,
    pronunciation_row: usize,
) {
    let mut phase_start = std::time::Instant::now();
    let mut phases = Vec::new();
    macro_rules! phase {
        ($name:literal) => {{
            phases.push(serde_json::json!({"phase":$name,"elapsed_us":phase_start.elapsed().as_micros()}));
            phase_start = std::time::Instant::now();
        }};
    }
    let row: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::var(receipt_variable).unwrap()).unwrap())
            .unwrap();
    assert_eq!(row["text"], text);
    assert_eq!(row["complete"], false);
    assert!(row["lexical_fact_refusal"].is_null());
    let bytes = hex(row["lexical_fact_bytes"].as_str().unwrap());
    let admitted = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
    // The producer checked this Source against imported Native schemas. Retain that
    // proof unchanged; freshly admit its exact primitive inputs against the full
    // generated Source closure. No nominal identity is rewritten.
    assert!(LanguageParserWindow8StableLexicalFact::from_structured(admitted.clone()).is_err());
    assert_eq!(admitted.canonical_bytes().unwrap(), bytes);
    phase!("read_original_checked_receipt");
    let original_query = field(&admitted, "query");
    let original_snapshot = field(original_query, "snapshot");
    let lexical_native =
        LanguageParserWindow8Lexical::from_structured(field(original_snapshot, "lexical").clone())
            .unwrap();
    let basis =
        LanguageParserBasis::from_structured(field(original_snapshot, "basis").clone()).unwrap();
    phase!("decode_exact_raw_lexical_and_basis");
    let fresh_snapshot = LanguageParserWindow8Snapshot::new(
        basis,
        readmit_checked(field(original_snapshot, "candidate0")),
        readmit_checked(field(original_snapshot, "candidate1")),
        readmit_checked(field(original_snapshot, "candidate2")),
        readmit_checked(field(original_snapshot, "candidate3")),
        lexical_native,
    )
    .unwrap();
    assert_eq!(
        fresh_snapshot.lexical().clone().into_structured().unwrap(),
        *field(original_snapshot, "lexical")
    );
    assert_eq!(
        fresh_snapshot.basis().clone().into_structured().unwrap(),
        *field(original_snapshot, "basis")
    );
    let conduit_core::StructuredInfoValueShape::Leaf(dependent_bytes) =
        field(original_query, "dependent").shape()
    else {
        panic!("ordinal required")
    };
    let dependent = u64::from_le_bytes(dependent_bytes.try_into().unwrap());
    phase!("admit_four_checked_hypotheses_and_snapshot");
    let fresh_query = LanguageParserWindow8FactQuery::new(dependent, fresh_snapshot).unwrap();
    let fact = LanguageParserWindow8StableLexicalFact::new(fresh_query).unwrap();
    phase!("admit_query_and_stable_lexical_fact");
    assert_eq!(*fact.query().dependent(), expected_dependent);
    let fresh_bytes = fact.clone().encode().unwrap();
    assert_eq!(
        LanguageParserWindow8StableLexicalFact::decode(&fresh_bytes).unwrap(),
        fact
    );
    phase!("generated_native_encode_decode_roundtrip");
    let snapshot = fact.query().snapshot();
    let native_tape = snapshot.lexical().tape();
    assert_eq!(
        *native_tape.source().finality(),
        LanguageTextFinality::Partial
    );
    assert_eq!(native_tape.source().material().text(), text);
    let hypotheses = [
        snapshot.candidate0().hypothesis(),
        snapshot.candidate1().hypothesis(),
        snapshot.candidate2().hypothesis(),
        snapshot.candidate3().hypothesis(),
    ];
    assert!(hypotheses.iter().all(|hypothesis| *hypothesis.active()));
    assert!(hypotheses
        .iter()
        .all(|hypothesis| *hypothesis.selected() > expected_dependent
            && hypothesis.choices()[expected_dependent as usize] == choice));
    if expected_dependent == 1 {
        assert!(hypotheses.iter().any(|hypothesis| {
            hypothesis.state().heads()[..native_tape.tokens().len()].contains(&9)
        }));
    }
    let lexical = conduit_language::lexical::prepare_lexical_tape(
        native_tape.source(),
        native_tape.profile(),
        None,
    )
    .unwrap();
    phase!("reprepare_exact_lexical_tape");
    let selection = prepare_stable_lexical_selection(&lexical, &fact).unwrap();
    assert!(std::ptr::eq(selection.fact(), &fact));
    assert_eq!(*selection.candidate().pos(), pos);
    assert_eq!(selection.candidate().lemma(), "record");
    phase!("public_language_candidate_selection");
    let profile = speech_fixture::profile();
    let pronunciation = prepare_stable_lexical_pronunciation(&selection, &profile).unwrap();
    assert!(std::ptr::eq(pronunciation.selection().fact(), &fact));
    assert_eq!(
        *pronunciation.row_selection().index(),
        pronunciation_row as u64
    );
    assert_eq!(
        pronunciation.result().phones(),
        profile.rows()[pronunciation_row].phones()
    );
    assert_ne!(
        pronunciation.result().phones(),
        profile.rows()[1 - pronunciation_row].phones()
    );
    assert_eq!(pronunciation.request().candidate(), selection.candidate());
    phase!("public_speech_pronunciation_lookup");
    let _ = phase_start;
    let receipt = serde_json::json!({
        "schema":"language/generated-early-pronunciation-readmission@1",
        "text":text,"dependent":dependent,"choice":choice,"pronunciation_row":pronunciation_row,
        "bridge":"fresh generated Source constructors; original imported-schema receipt unchanged",
        "additional_model_invocations":0,"played_authority":false,"phases":phases,
        "original_checked_fact_bytes":bytes.len(),"generated_fact_bytes":fresh_bytes.len(),
    });
    eprintln!("{receipt}");
    if let Ok(directory) = std::env::var("CONDUIT_EARLY_PRONUNCIATION_OUTPUT") {
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(directory.join("generated-lexical-fact.bin"), &fresh_bytes).unwrap();
        std::fs::write(
            directory.join("pronunciation-request.bin"),
            pronunciation.request().clone().encode().unwrap(),
        )
        .unwrap();
        std::fs::write(
            directory.join("pronunciation-result.bin"),
            pronunciation.result().clone().encode().unwrap(),
        )
        .unwrap();
        std::fs::write(
            directory.join("readmission.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
}

#[test]
#[ignore = "requires original actual record pair via CONDUIT_AVAILABLE_RECORD_PAIR_RECEIPTS"]
fn actual_available_noun_disagreement_refuses_fresh_generated_stable_fact() {
    let rows: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("CONDUIT_AVAILABLE_RECORD_PAIR_RECEIPTS").unwrap()).unwrap(),
    )
    .unwrap();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["text"] == "I record the record ")
        .unwrap();
    assert_eq!(row["complete"], false);
    let bytes = hex(row["lexical_fact_bytes"].as_str().unwrap());
    let original = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
    let old_snapshot = field(field(&original, "query"), "snapshot");
    let snapshot = LanguageParserWindow8Snapshot::new(
        LanguageParserBasis::from_structured(field(old_snapshot, "basis").clone()).unwrap(),
        readmit_checked(field(old_snapshot, "candidate0")),
        readmit_checked(field(old_snapshot, "candidate1")),
        readmit_checked(field(old_snapshot, "candidate2")),
        readmit_checked(field(old_snapshot, "candidate3")),
        LanguageParserWindow8Lexical::from_structured(field(old_snapshot, "lexical").clone())
            .unwrap(),
    )
    .unwrap();
    let hypotheses = [
        snapshot.candidate0(),
        snapshot.candidate1(),
        snapshot.candidate2(),
        snapshot.candidate3(),
    ];
    assert!(hypotheses
        .iter()
        .all(|candidate| *candidate.hypothesis().active()
            && *candidate.hypothesis().selected() == 4
            && *candidate.hypothesis().state().unread() == 4));
    assert_eq!(
        hypotheses.map(|candidate| candidate.hypothesis().choices()[3]),
        [0, 0, 0, 1]
    );
    let query = LanguageParserWindow8FactQuery::new(3, snapshot).unwrap();
    assert!(matches!(
        LanguageParserWindow8StableLexicalFact::new(query),
        Err(conduit_plot::rust_binding::NativeBindingRefusal::ViolatedInvariant { .. })
    ));
    assert_eq!(original.canonical_bytes().unwrap(), bytes);
}
