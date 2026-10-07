#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
use conduit_core::StructuredInfoValue;
use conduit_language::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[allow(dead_code)]
#[path = "common/parser_joint_flows.rs"]
mod parser_joint_flows;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/scorer_model.rs"]
mod scorer_model;

fn bind<T: NativeRustBinding>(fields: Vec<(&str, StructuredInfoValue)>) -> T {
    T::from_structured(fixture::record(&T::semantic_type().unwrap(), fields)).unwrap()
}
fn source() -> String {
    [
        joint::source(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_branch.conduit").into(),
        include_str!("../parser_session_protection.conduit").into(),
        include_str!("../parser_session_protected_set.conduit").into(),
        include_str!("../parser_session_protected_branch.conduit").into(),
        include_str!("../parser_session_protected_mask.conduit").into(),
    ]
    .join("\n")
}
fn refusal<T: NativeRustBinding>(value: StructuredInfoValue) {
    assert!(matches!(
        T::from_structured(value),
        Err(NativeBindingRefusal::ViolatedInvariant { .. })
    ));
}

#[test]
#[ignore = "actual fact custody and ordinary Source replay; no fresh learned or played claim"]
fn source_owned_slots_refuse_foreign_choices_and_filter_legal_proposals() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../training/ewt_joint_v2/native_stream_protected_projection_receipt.json"
    ))
    .unwrap();
    let decode = |name: &str| {
        StructuredInfoValue::from_canonical_bytes(
            &serde_json::from_value::<Vec<u8>>(receipt[name].clone()).unwrap(),
        )
        .unwrap()
    };
    let fact = LanguageParserJointStableFact::from_structured(decode("fact_bytes")).unwrap();
    let edge =
        LanguageParserProtectedEdgeProposal::from_structured(decode("compact_proposal_bytes"))
            .unwrap();
    let context =
        LanguageParserProtectedProjectionContext::from_structured(decode("native_context_bytes"))
            .unwrap();
    assert_eq!(*edge.dependent(), 0);
    assert_eq!(*edge.head(), 1);
    let entries = [
        "language-parser-protected-set-initialize",
        "language-parser-protected-set-insert",
        "language-parser-initialize",
        "language-parser-independent-branch",
        "language-parser-legal-mask",
        "language-parser-independent-mask",
    ];
    let blueprints: Vec<_> = entries
        .iter()
        .map(|entry| {
            eprintln!("protection preparation start: {entry}");
            let blueprint = parser_kernel::Blueprint::prepare(source(), entry);
            eprintln!("protection preparation complete: {entry}");
            blueprint
        })
        .collect();
    eprintln!("protection ordinary Plan preparation start");
    let mut flows = parser_joint_flows::Pipelines::new(&blueprints, 0);
    eprintln!("protection ordinary Plan preparation complete");
    let retained = LanguageParserProtectedSetProposal::from_structured(
        flows.call(0, &edge.clone().into_structured().unwrap()),
    )
    .unwrap();
    assert_eq!(*retained.active(), [true, false, false, false]);
    assert_eq!(retained.edge0(), &edge);
    let insert: LanguageParserProtectedInsertContext = bind(vec![
        ("context", context.into_structured().unwrap()),
        ("previous", retained.clone().into_structured().unwrap()),
    ]);
    let inserted = LanguageParserProtectedSetProposal::from_structured(
        flows.call(1, &insert.clone().into_structured().unwrap()),
    )
    .unwrap();
    assert_eq!(inserted, retained);
    let insert_value = insert.into_structured().unwrap();
    let changed_edge = fixture::replace(
        &edge.clone().into_structured().unwrap(),
        "head",
        fixture::number(
            fixture::field_type(
                &LanguageParserProtectedEdgeProposal::semantic_type().unwrap(),
                "head",
            ),
            4,
        ),
    );
    let changed_set = fixture::replace(
        &retained.clone().into_structured().unwrap(),
        "edge0",
        changed_edge,
    );
    refusal::<LanguageParserProtectedInsertContext>(fixture::replace(
        &insert_value,
        "previous",
        changed_set,
    ));
    let basis = fact.query().beam().basis().clone();
    let default_relation = fact
        .query()
        .beam()
        .candidate0()
        .parser()
        .state()
        .relation2()
        .clone();
    let begin_ty = LanguageParserBegin::semantic_type().unwrap();
    let begin: LanguageParserBegin = bind(vec![
        ("basis", basis.clone().into_structured().unwrap()),
        (
            "default_relation",
            default_relation.clone().into_structured().unwrap(),
        ),
        (
            "token_count",
            fixture::number(fixture::field_type(&begin_ty, "token_count"), 2),
        ),
    ]);
    let initial = flows.call(2, &begin.into_structured().unwrap());
    let state = LanguageParserState::from_structured(joint::retype(
        &LanguageParserState::semantic_type().unwrap(),
        &initial,
    ))
    .unwrap();
    let lexical = fact.query().beam().lexical().clone();
    let seed = LanguageParserJointRuntimeHypothesis::new(
        LanguageParserJointHypothesis::new(
            [0; 4],
            LanguageParserHypothesis::new(true, 0, 0, state.clone()).unwrap(),
        )
        .unwrap(),
        0,
    )
    .unwrap();
    let mut branch_refusals = 0;
    for choice in 0..4 {
        let branch = LanguageParserJointProtectedBranchQuery::new(
            LanguageParserJointBranchQuery::new(choice, seed.clone(), lexical.clone()).unwrap(),
        )
        .unwrap();
        let raw = fixture::record(
            &LanguageParserIndependentBranchContext::semantic_type().unwrap(),
            vec![
                ("branch", branch.into_structured().unwrap()),
                ("retained", retained.clone().into_structured().unwrap()),
            ],
        );
        if choice == *edge.dependent_choice() {
            let admitted = LanguageParserIndependentBranchContext::from_structured(raw).unwrap();
            let result = LanguageParserJointBranchResult::from_structured(
                flows.call(3, &admitted.into_structured().unwrap()),
            )
            .unwrap();
            assert!(*result.accepted());
            assert_eq!(result.proposal().hypothesis().choices()[0], choice);
        } else {
            refusal::<LanguageParserIndependentBranchContext>(raw);
            branch_refusals += 1;
        }
    }
    assert_eq!(branch_refusals, 3);
    let mask_query = LanguageParserMaskQuery::new(
        basis.clone(),
        LanguageParserNumericState::from_structured(initial).unwrap(),
    )
    .unwrap();
    let legal = LanguageParserLegalMask::from_structured(
        flows.call(4, &mask_query.clone().into_structured().unwrap()),
    )
    .unwrap();
    let query: LanguageParserIndependentMaskQuery = bind(vec![
        (
            "default_relation",
            default_relation.clone().into_structured().unwrap(),
        ),
        ("mask", legal.clone().into_structured().unwrap()),
        ("request", mask_query.into_structured().unwrap()),
        ("retained", retained.clone().into_structured().unwrap()),
    ]);
    let filtered =
        LanguageParserLegalMask::from_structured(flows.call(5, &query.into_structured().unwrap()))
            .unwrap();
    assert!(
        legal.allowed()[73],
        "Source class73 is right/root at the root spine"
    );
    assert!(filtered.allowed()[0]);
    assert!(filtered.allowed()[39..].iter().all(|allowed| !allowed));
    assert_eq!(filtered.state(), legal.state());
    assert_eq!(filtered.basis(), legal.basis());
}
