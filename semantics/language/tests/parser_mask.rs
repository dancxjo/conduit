use conduit_ai::integer_masked_rank::integer_masked_top_k;
use conduit_core::*;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
use fixture::*;

fn boolean(value: &StructuredInfoValue) -> bool {
    match value.shape() {
        StructuredInfoValueShape::Leaf([0]) => false,
        StructuredInfoValueShape::Leaf([1]) => true,
        _ => panic!("exact Boolean"),
    }
}
fn numbers_like(value: &StructuredInfoValue, numbers: [u64; 5]) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Collection { element, .. } = value.value_type().shape() else {
        panic!("numeric collection")
    };
    StructuredInfoValue::collection(
        value.value_type().clone(),
        numbers.into_iter().map(|n| number(element, n)).collect(),
    )
    .unwrap()
}
fn scalar(state: &StructuredInfoValue, name: &str, value: u64) -> StructuredInfoValue {
    replace(state, name, number(field(state, name).value_type(), value))
}
fn class_request(
    f: &Fixture,
    state: &StructuredInfoValue,
    basis: &StructuredInfoValue,
    class: u64,
) -> StructuredInfoValue {
    let ty = f.ty("LanguageParserScoredClass");
    record(
        ty,
        vec![
            ("basis", basis.clone()),
            ("state", state.clone()),
            ("class", number(field_type(ty, "class"), class)),
            (
                "score",
                StructuredInfoValue::leaf(
                    field_type(ty, "score").clone(),
                    0i64.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            ("default_relation", f.relation("dep")),
        ],
    )
}

#[test]
fn source_mask_matches_actual_transition_acceptance_for_every_action_class() {
    let mut f = Fixture::new();
    let source = parser_source();
    let mut masks = parser_kernel::Execution::prepare(source.clone(), "language-parser-legal-mask");
    let mut proposals =
        parser_kernel::Execution::prepare(source.clone(), "language-parser-score-proposal");
    let mut transitions = parser_kernel::Execution::prepare(source, "language-parser-transition");
    masks.kernel.start().unwrap();
    proposals.kernel.start().unwrap();
    transitions.kernel.start().unwrap();
    let initial = f.initial(4);
    let shifted = field(&f.step(&initial, "shift", "dep", "analysis/1"), "state").clone();
    let rooted = field(
        &f.step(&initial, "right_arc", "root", "analysis/1"),
        "state",
    )
    .clone();
    let committed = scalar(&rooted, "committed", 1);
    let single = f.initial(1);
    let dead_end = field(&f.step(&single, "shift", "dep", "analysis/1"), "state").clone();
    // A prior acyclic graph may contain a path to unread; adding its reverse
    // arc must be refused by both the source mask and actual transition graph.
    let mut cyclic_proposal = f.initial(3);
    cyclic_proposal = scalar(&cyclic_proposal, "unread", 2);
    cyclic_proposal = scalar(&cyclic_proposal, "depth", 2);
    cyclic_proposal = replace(
        &cyclic_proposal,
        "stack",
        numbers_like(field(&cyclic_proposal, "stack"), [4, 0, 4, 4, 4]),
    );
    cyclic_proposal = replace(
        &cyclic_proposal,
        "heads",
        numbers_like(field(&cyclic_proposal, "heads"), [1, 2, 5, 5, 4]),
    );
    assert!(f.native_ok(&cyclic_proposal));
    let invalid = scalar(&rooted, "committed", 4);
    assert!(!f.native_ok(&invalid));
    let foreign = replace(
        field(&initial, "basis"),
        "analysis_revision",
        text(
            field(field(&initial, "basis"), "analysis_revision").value_type(),
            "foreign-analysis",
        ),
    );
    let unread_five = scalar(&initial, "unread", 5);
    let unread_max = scalar(&initial, "unread", u64::MAX);
    let invalid_top = replace(
        &scalar(&initial, "depth", 2),
        "stack",
        numbers_like(field(&initial, "stack"), [4, u64::MAX, 4, 4, 4]),
    );
    for invalid in [&unread_five, &unread_max, &invalid_top] {
        assert!(!f.native_ok(invalid));
    }
    let states = [
        (initial.clone(), field(&initial, "basis").clone()),
        (shifted.clone(), field(&shifted, "basis").clone()),
        (rooted.clone(), field(&rooted, "basis").clone()),
        (committed.clone(), field(&committed, "basis").clone()),
        (dead_end.clone(), field(&dead_end, "basis").clone()),
        (
            cyclic_proposal.clone(),
            field(&cyclic_proposal, "basis").clone(),
        ),
        (invalid.clone(), field(&invalid, "basis").clone()),
        (initial, foreign),
        (unread_five.clone(), field(&unread_five, "basis").clone()),
        (unread_max.clone(), field(&unread_max, "basis").clone()),
        (invalid_top.clone(), field(&invalid_top, "basis").clone()),
    ];
    let mut sequence = 0;
    for (case, (state, basis)) in states.into_iter().enumerate() {
        let ty = f.ty("LanguageParserMaskQuery");
        let query = record(ty, vec![("basis", basis.clone()), ("state", state.clone())]);
        let masked = masks.transact(case as u64, &query);
        assert_eq!(field(&masked, "basis"), &basis);
        assert_eq!(field(&masked, "state"), &state);
        let allowed = (0..76)
            .map(|class| boolean(index(field(&masked, "allowed"), class)))
            .collect::<Vec<_>>();
        for class in 0..76 {
            let proposed = proposals.transact(sequence, &class_request(&f, &state, &basis, class));
            let result = transitions.transact(sequence, field(&proposed, "request"));
            assert_eq!(
                allowed[class as usize],
                accepted(&result),
                "case={case} class={class}"
            );
            if accepted(&result) {
                assert!(f.native_ok(field(&result, "state")));
            } else {
                assert_eq!(field(&result, "state"), &state);
                if case == 6 || case >= 8 {
                    assert_eq!(tag(field(&result, "refusal")), "invalid_state");
                }
            }
            sequence += 1;
        }
        // Numeric ranking has no parser policy: an arbitrarily high illegal
        // score is ignored solely because the checked source mask rejects it.
        let scores = (0..76).map(|i| i as i64).collect::<Vec<_>>();
        let selected = integer_masked_top_k(&scores, &allowed, 4).unwrap();
        assert!(selected.len() <= 4);
        assert!(selected.iter().all(|index| allowed[*index]));
        if case >= 6 || case == 4 {
            assert!(selected.is_empty());
        }
    }
    assert_eq!(sequence, 836);
}
