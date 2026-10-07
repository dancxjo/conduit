//! Dynamic Source-law checking leaves the frozen generated library unchanged.
use conduit_core::*;
use conduit_language::{
    parser_window8::{self, lexical::*},
    *,
};
use conduit_plot::rust_binding::{validate_native_invariants, BoundedSequence, NativeRustBinding};
use conduit_plot::*;

#[path = "common/window8_dynamic_values.rs"]
mod values;
use values::*;
#[path = "common/window8_lexical_ledger.rs"]
mod ledger;

#[test]
fn source_ledger_retains_two_exact_revision_hops() {
    let startup = StartupCatalog::new();
    let source = [
        include_str!("../identity.conduit"),
        include_str!("../types.conduit"),
        include_str!("../text_revision.conduit"),
        include_str!("../revision_lineage.conduit"),
        include_str!("../lexical.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_beam.conduit"),
        include_str!("../parser_scorer.conduit"),
        include_str!("../parser_mask.conduit"),
        include_str!("../parser_window8.conduit"),
        include_str!("../parser_window8_search.conduit"),
        include_str!("../parser_window8_facts.conduit"),
        include_str!("../parser_window8_reanalysis.conduit"),
        include_str!("../parser_window8_lexical_protection.conduit"),
        include_str!("../parser_window8_lexical_ledger.conduit"),
    ]
    .join("\n");
    eprintln!("window8 lexical-fact: dynamic Source check start");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
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
        &LanguageParserWindow8Begin::new(basis.clone(), relation.clone(), 1).unwrap(),
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
    let snapshot = |choices: [u64; 4],
                    lexical: &LanguageParserWindow8Lexical,
                    offered_basis: &LanguageParserBasis| {
        record(
            snapshot_ty,
            vec![
                ("lexical", lexical.clone().into_structured().unwrap()),
                ("basis", offered_basis.clone().into_structured().unwrap()),
                ("candidate0", hypothesis(choices[0])),
                ("candidate1", hypothesis(choices[1])),
                ("candidate2", hypothesis(choices[2])),
                ("candidate3", hypothesis(choices[3])),
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
    let agreed_snapshot = snapshot([0; 4], lexical.lexical(), &basis);
    validate_native_invariants(&agreed_snapshot, &snapshot_ty.invariants).unwrap();
    let agreed = record(fact, vec![("query", query(agreed_snapshot.clone()))]);
    validate_native_invariants(&agreed, &fact.invariants).unwrap();
    // The root/dependency remains unassigned: lexical agreement is independent.
    assert_eq!(state.state().heads()[0], 9);
    let verb_snapshot = snapshot([1; 4], lexical.lexical(), &basis);
    validate_native_invariants(&verb_snapshot, &snapshot_ty.invariants).unwrap();
    validate_native_invariants(
        &record(fact, vec![("query", query(verb_snapshot))]),
        &fact.invariants,
    )
    .unwrap();
    let different_snapshot = snapshot([0, 1, 0, 0], lexical.lexical(), &basis);
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
        &snapshot([0; 4], lexical.lexical(), &foreign_basis),
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
    let unstable_snapshot = snapshot([0; 4], unstable.lexical(), &basis);
    validate_native_invariants(&unstable_snapshot, &snapshot_ty.invariants).unwrap();
    assert!(validate_native_invariants(
        &record(fact, vec![("query", query(unstable_snapshot))]),
        &fact.invariants
    )
    .is_err());
    let next = LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            source.material().identity().clone(),
            source.material().language().clone(),
            LanguageTextRevisionId::new("window8/fact/r1".into()).unwrap(),
            "record record ".into(),
        )
        .unwrap(),
        Some(LanguageTextPriorRevision::new(source.material().revision().clone(), 0).unwrap()),
        source.provenance().clone(),
        1,
        Some(6),
    )
    .unwrap();
    let next_tape =
        conduit_language::lexical::prepare_lexical_tape(&next, &profile, Some(&tape)).unwrap();
    let next_lexical = prepare_window8_lexical(&next_tape).unwrap();
    let next_basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/fact-reanalysis1".into()).unwrap(),
        next.material().revision().clone(),
        next.material().identity().clone(),
    )
    .unwrap();
    let lineage = prepare_text_revision_lineage(&source, &next).unwrap();
    let protection_ty = native(&checked, "LanguageParserWindow8ProtectedLexicalChoice");
    let receipt = record(
        protection_ty,
        vec![
            ("fact", agreed.clone()),
            (
                "next",
                next_lexical.lexical().clone().into_structured().unwrap(),
            ),
            ("lineage", lineage.clone().into_structured().unwrap()),
            ("origin_basis", basis.clone().into_structured().unwrap()),
            (
                "current_basis",
                next_basis.clone().into_structured().unwrap(),
            ),
        ],
    );
    validate_native_invariants(&receipt, &protection_ty.invariants).unwrap();
    let branch_ty = native(&checked, "LanguageParserWindow8ProtectedChoiceBranch");
    let branch = |dependent, choice| {
        record(
            branch_ty,
            vec![
                ("receipt", receipt.clone()),
                ("dependent", scalar(branch_ty, "dependent", dependent)),
                ("choice", scalar(branch_ty, "choice", choice)),
                ("basis", next_basis.clone().into_structured().unwrap()),
            ],
        )
    };
    let program = checked_program(&checked, "language-window8-protected-choice");
    for (dependent, choice, allowed) in [(0, 0, true), (0, 1, false), (1, 1, true)] {
        let input = branch(dependent, choice);
        validate_native_invariants(&input, &branch_ty.invariants).unwrap();
        let output = StructuredInfoValue::from_canonical_bytes(
            &program.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            field(&output, "allowed").shape(),
            StructuredInfoValueShape::Leaf(&[u8::from(allowed)])
        );
    }

    let mut ledger = ledger::Ledger::prepare(&checked);
    ledger.insert(receipt.clone()).unwrap();
    assert!(ledger
        .allows(0, 0, next_basis.clone().into_structured().unwrap())
        .unwrap());
    assert!(!ledger
        .allows(0, 1, next_basis.clone().into_structured().unwrap())
        .unwrap());
    let third = LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            next.material().identity().clone(),
            next.material().language().clone(),
            LanguageTextRevisionId::new("window8/fact/r2".into()).unwrap(),
            "record record record ".into(),
        )
        .unwrap(),
        Some(LanguageTextPriorRevision::new(next.material().revision().clone(), 1).unwrap()),
        next.provenance().clone(),
        2,
        Some(6),
    )
    .unwrap();
    let third_tape =
        conduit_language::lexical::prepare_lexical_tape(&third, &profile, Some(&next_tape))
            .unwrap();
    let third_lexical = prepare_window8_lexical(&third_tape).unwrap();
    let third_basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/fact-reanalysis2".into()).unwrap(),
        third.material().revision().clone(),
        third.material().identity().clone(),
    )
    .unwrap();
    let second_lineage = prepare_text_revision_lineage(&next, &third).unwrap();
    let before = ledger.evidence();
    assert!(ledger
        .rebase(
            third_lexical.lexical().clone().into_structured().unwrap(),
            second_lineage.clone().into_structured().unwrap(),
            basis.clone().into_structured().unwrap()
        )
        .is_err());
    assert_eq!(ledger.evidence(), before);
    ledger
        .rebase(
            third_lexical.lexical().clone().into_structured().unwrap(),
            second_lineage.clone().into_structured().unwrap(),
            third_basis.clone().into_structured().unwrap(),
        )
        .unwrap();
    assert!(ledger
        .allows(0, 0, third_basis.clone().into_structured().unwrap())
        .unwrap());
    assert!(!ledger
        .allows(0, 1, third_basis.clone().into_structured().unwrap())
        .unwrap());
    let evidence = ledger.evidence();
    assert_eq!(evidence[0].0, agreed);
    assert_eq!(evidence[0].2.len(), 2);
    let anchor = field(&evidence[0].1, "anchor");
    assert_eq!(
        field(&anchor, "origin_basis"),
        basis.clone().into_structured().unwrap()
    );
    assert_eq!(
        field(&anchor, "current_basis"),
        third_basis.clone().into_structured().unwrap()
    );
    assert_eq!(
        field(&anchor, "occurrence"),
        third_lexical.lexical().tape().tokens()[0]
            .identity()
            .clone()
            .into_structured()
            .unwrap()
    );
    assert_eq!(
        third_lexical.lexical().tape().tokens()[0].prior_occurrence(),
        &Some(next_lexical.lexical().tape().tokens()[0].identity().clone())
    );
    assert!(ledger
        .allows(0, 0, next_basis.clone().into_structured().unwrap())
        .is_err());
    // Capacity fixture uses repeated exact origins solely to exercise bounded
    // custody; it does not claim eight distinct learned stable occurrences.
    let mut bounded = ledger::Ledger::prepare(&checked);
    for _ in 0..8 {
        bounded.insert(receipt.clone()).unwrap();
    }
    let full = bounded.evidence();
    assert!(bounded.insert(receipt).is_err());
    assert_eq!(bounded.evidence(), full);
    let bad_receipt = replace(
        &before[0].2[0],
        "origin_basis",
        third_basis.clone().into_structured().unwrap(),
    );
    let mut refused = ledger::Ledger::prepare(&checked);
    assert!(refused.insert(bad_receipt).is_err());
    assert!(refused.evidence().is_empty());
    let context_ty = native(&checked, "LanguageParserWindow8LexicalAnchorContext");
    let forged_projection = record(
        context_ty,
        vec![
            ("receipt", before[0].2[0].clone()),
            ("choice", scalar(context_ty, "choice", 0)),
            (
                "occurrence",
                next_lexical.lexical().tape().tokens()[1]
                    .identity()
                    .clone()
                    .into_structured()
                    .unwrap(),
            ),
        ],
    );
    assert!(validate_native_invariants(&forged_projection, &context_ty.invariants).is_err());
    let unlinked_tokens =
        BoundedSequence::try_from_iter(third_lexical.lexical().tape().tokens().iter().map(|t| {
            LanguageLexicalToken::new(
                t.candidates().clone(),
                *t.category(),
                *t.completeness(),
                t.identity().clone(),
                None,
                t.span().clone(),
                t.surface().clone(),
            )
            .unwrap()
        }))
        .unwrap();
    let unlinked_tape =
        LanguageLexicalTape::new(profile.clone(), third.clone(), unlinked_tokens).unwrap();
    let unlinked = replace(
        &third_lexical.lexical().clone().into_structured().unwrap(),
        "tape",
        unlinked_tape.into_structured().unwrap(),
    );
    let mut second_refusal = ledger::Ledger::prepare(&checked);
    second_refusal.insert(before[0].2[0].clone()).unwrap();
    let retained = second_refusal.evidence();
    assert!(second_refusal
        .rebase(
            unlinked,
            second_lineage.into_structured().unwrap(),
            third_basis.into_structured().unwrap()
        )
        .is_err());
    assert_eq!(second_refusal.evidence(), retained);
}
