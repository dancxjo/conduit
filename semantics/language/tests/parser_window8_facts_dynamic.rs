//! Dynamic Source-law checking leaves the frozen generated library unchanged.
use conduit_core::{StructuredFieldValue, StructuredInfoValue};
use conduit_language::{
    parser_window8::{self, lexical::*},
    *,
};
use conduit_plot::rust_binding::{validate_native_invariants, BoundedSequence, NativeRustBinding};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

fn record(
    ty: &conduit_plot::CheckedNativeType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.value_type.clone(),
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
fn native<'a>(
    checked: &'a conduit_plot::CheckedSyntaxDocument,
    name: &str,
) -> &'a conduit_plot::CheckedNativeType {
    checked
        .native_types
        .iter()
        .find(|ty| ty.name == name)
        .unwrap()
}

#[test]
fn lexical_fact_source_checks_against_installed_exact_native_schemas() {
    let mut startup = StartupCatalog::new();
    for (name, value_type) in [
        (
            "LanguageParserWindow8RawHypothesis",
            LanguageParserWindow8RawHypothesis::semantic_type().unwrap(),
        ),
        (
            "LanguageParserWindow8StateProof",
            LanguageParserWindow8StateProof::semantic_type().unwrap(),
        ),
        (
            "LanguageParserWindow8Lexical",
            LanguageParserWindow8Lexical::semantic_type().unwrap(),
        ),
        (
            "LanguageParserBasis",
            LanguageParserBasis::semantic_type().unwrap(),
        ),
    ] {
        startup.ensure_structured_type(name, value_type).unwrap();
    }
    let source = include_str!("../parser_window8_facts.conduit");
    eprintln!("window8 lexical-fact: dynamic Source check start");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    eprintln!("window8 lexical-fact: dynamic Source check complete");
    let fact = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "LanguageParserWindow8StableLexicalFact")
        .unwrap();
    assert!(!fact.invariants.is_empty());

    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "window8/fact-fixture".into(),
        "profile/3".into(),
    )
    .unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            LanguageTextId::new("window8/fact".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("window8/fact/r0".into()).unwrap(),
            "record ".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        Some(6),
    )
    .unwrap();
    let candidates = BoundedSequence::try_from_iter(
        [LanguageLexicalPos::Noun, LanguageLexicalPos::Verb]
            .into_iter()
            .map(|pos| {
                LanguageLexicalCandidate::new("record".into(), BoundedSequence::new(), pos).unwrap()
            }),
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([
            LanguageLexicalEntry::new(candidates, "record".into()).unwrap()
        ])
        .unwrap(),
        "window8/fact-profile".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance,
    )
    .unwrap();
    let tape = conduit_language::lexical::prepare_lexical_tape(&source, &profile, None).unwrap();
    let lexical = prepare_window8_lexical(&tape).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/fact-analysis".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let state = parser_window8::initialize_window8(
        &LanguageParserWindow8Begin::new(basis.clone(), relation, 1).unwrap(),
    )
    .unwrap();
    let hypothesis_ty = native(&checked, "LanguageParserWindow8CheckedHypothesis");
    let hypothesis = |choice: u64| {
        let mut choices = [0; 8];
        choices[0] = choice;
        let raw = LanguageParserWindow8RawHypothesis::new(
            true,
            choices,
            choice,
            0,
            1,
            state.state().clone(),
        )
        .unwrap();
        let value = record(
            hypothesis_ty,
            vec![
                ("hypothesis", raw.into_structured().unwrap()),
                ("proof", state.proof().clone().into_structured().unwrap()),
            ],
        );
        validate_native_invariants(&value, &hypothesis_ty.invariants).unwrap();
        value
    };
    let snapshot_ty = native(&checked, "LanguageParserWindow8Snapshot");
    let snapshot = |second: u64,
                    lexical: &LanguageParserWindow8Lexical,
                    offered_basis: &LanguageParserBasis| {
        record(
            snapshot_ty,
            vec![
                ("lexical", lexical.clone().into_structured().unwrap()),
                ("basis", offered_basis.clone().into_structured().unwrap()),
                ("candidate0", hypothesis(0)),
                ("candidate1", hypothesis(second)),
                ("candidate2", hypothesis(0)),
                ("candidate3", hypothesis(0)),
            ],
        )
    };
    let query_ty = native(&checked, "LanguageParserWindow8FactQuery");
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = query_ty.value_type.shape()
    else {
        panic!("query record")
    };
    let ordinal_type = fields
        .iter()
        .find(|field| field.name() == "dependent")
        .unwrap()
        .value_type()
        .clone();
    let query = |snapshot| {
        let value = record(
            query_ty,
            vec![
                ("snapshot", snapshot),
                (
                    "dependent",
                    StructuredInfoValue::leaf(ordinal_type.clone(), 0u64.to_le_bytes().to_vec())
                        .unwrap(),
                ),
            ],
        );
        validate_native_invariants(&value, &query_ty.invariants).unwrap();
        value
    };
    let agreed_snapshot = snapshot(0, lexical.lexical(), &basis);
    validate_native_invariants(&agreed_snapshot, &snapshot_ty.invariants).unwrap();
    let agreed = record(fact, vec![("query", query(agreed_snapshot))]);
    validate_native_invariants(&agreed, &fact.invariants).unwrap();
    let different_snapshot = snapshot(1, lexical.lexical(), &basis);
    validate_native_invariants(&different_snapshot, &snapshot_ty.invariants).unwrap();
    let disagreement = record(fact, vec![("query", query(different_snapshot))]);
    assert!(validate_native_invariants(&disagreement, &fact.invariants).is_err());
    let foreign_basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/foreign-analysis".into()).unwrap(),
        basis.source_revision().clone(),
        basis.text().clone(),
    )
    .unwrap();
    assert!(validate_native_invariants(
        &snapshot(0, lexical.lexical(), &foreign_basis),
        &snapshot_ty.invariants
    )
    .is_err());
    let unstable_source = LanguageTextRevision::new(
        *source.finality(),
        source.material().clone(),
        None,
        source.provenance().clone(),
        0,
        Some(0),
    )
    .unwrap();
    let unstable_tape =
        conduit_language::lexical::prepare_lexical_tape(&unstable_source, &profile, None).unwrap();
    let unstable = prepare_window8_lexical(&unstable_tape).unwrap();
    let unstable_snapshot = snapshot(0, unstable.lexical(), &basis);
    validate_native_invariants(&unstable_snapshot, &snapshot_ty.invariants).unwrap();
    assert!(validate_native_invariants(
        &record(fact, vec![("query", query(unstable_snapshot))]),
        &fact.invariants
    )
    .is_err());
}
