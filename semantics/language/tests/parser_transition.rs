use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "common/parser_fixture.rs"]
mod fixture;
use fixture::*;

#[test]
fn root_vocative_and_reduce_are_source_owned_transitions() {
    let mut f = Fixture::new();
    let mut state = f.initial(2);
    for (action, relation) in [
        ("right_arc", "root"),
        ("right_arc", "vocative"),
        ("reduce", "dep"),
        ("reduce", "dep"),
    ] {
        let result = f.step(&state, action, relation, "analysis/1");
        assert!(accepted(&result));
        state = field(&result, "state").clone();
    }
    assert_eq!(count(field(&state, "unread")), 2);
    assert_eq!(count(field(&state, "depth")), 1);
    assert_eq!(count(index(field(&state, "heads"), 0)), 4);
    assert_eq!(count(index(field(&state, "heads"), 1)), 0);
    assert_eq!(tag(field(field(&state, "relation1"), "base")), "vocative");
    let projected = f.project(&state, 1);
    assert_eq!(tag(&projected), "assigned");
    let StructuredInfoValueShape::Variant { payload, .. } = projected.shape() else {
        panic!("arc")
    };
    let basis = field(payload, "basis");
    let token = |ordinal| {
        conduit_language::LanguageAnalysisTokenRef::new(
            conduit_language::LanguageAnalysisRevisionId::from_structured(
                field(basis, "analysis_revision").clone(),
            )
            .unwrap(),
            conduit_language::LinguisticTokenIdentity::new(
                ordinal,
                conduit_language::LanguageTextId::from_structured(field(basis, "text").clone())
                    .unwrap(),
                conduit_language::LanguageTextRevisionId::from_structured(
                    field(basis, "source_revision").clone(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let governor = token(count(field(payload, "head")));
    let arc = conduit_language::LanguageDependencyArc::new(
        token(count(field(payload, "dependent"))),
        conduit_language::LanguageDependencyHead::token(
            governor.revision().clone(),
            governor.token().clone(),
        )
        .unwrap(),
        conduit_language::LanguageDependencyRelation::new(
            conduit_language::LanguageUniversalDependencyRelation::from_structured(
                field(field(payload, "relation"), "base").clone(),
            )
            .unwrap(),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(*arc.dependent().token().ordinal(), 1);
    assert_eq!(
        arc.dependent().token().text_identity(),
        &conduit_language::LanguageTextId::new("utterance".into()).unwrap()
    );
}
#[test]
fn headless_shift_can_receive_a_left_arc_before_input_end() {
    let mut f = Fixture::new();
    let state = f.initial(3);
    let shifted = f.step(&state, "shift", "dep", "analysis/1");
    assert!(accepted(&shifted));
    let linked = f.step(field(&shifted, "state"), "left_arc", "nsubj", "analysis/1");
    assert!(accepted(&linked));
    let state = field(&linked, "state");
    assert_eq!(count(field(state, "unread")), 1);
    assert_eq!(count(index(field(state, "heads"), 0)), 1);
}
#[test]
fn illegal_or_stale_proposals_preserve_state() {
    let mut f = Fixture::new();
    let state = f.initial(1);
    for (action, relation, revision) in [
        ("reduce", "dep", "analysis/1"),
        ("left_arc", "obj", "analysis/1"),
        ("right_arc", "vocative", "analysis/1"),
        ("shift", "dep", "analysis/2"),
    ] {
        let result = f.step(&state, action, relation, revision);
        assert!(!accepted(&result));
        assert_eq!(field(&result, "state"), &state);
    }
    let shifted = f.step(&state, "shift", "dep", "analysis/1");
    let state = field(&shifted, "state");
    for action in ["shift", "reduce", "left_arc", "right_arc"] {
        let result = f.step(state, action, "dep", "analysis/1");
        assert!(!accepted(&result));
        assert_eq!(field(&result, "state"), state);
    }
}

#[test]
fn commitment_and_finite_token_bounds_refuse_mutation() {
    let mut f = Fixture::new();
    let state = f.initial(4);
    assert_eq!(tag(&f.project(&state, 0)), "unassigned");
    let shifted = f.step(&state, "shift", "dep", "analysis/1");
    let state = field(&shifted, "state");
    let initial = f.initial(4);
    let root = f.step(&initial, "right_arc", "root", "analysis/1");
    let root_state = field(&root, "state");
    let committed = replace(
        root_state,
        "committed",
        number(field(root_state, "committed").value_type(), 1),
    );
    let refused = f.step(&committed, "left_arc", "nsubj", "analysis/1");
    assert!(!accepted(&refused));
    assert_eq!(field(&refused, "state"), &committed);
    let mut state = state.clone();
    for _ in 1..4 {
        let result = f.step(&state, "shift", "dep", "analysis/1");
        assert!(accepted(&result));
        state = field(&result, "state").clone();
    }
    let refused = f.step(&state, "shift", "dep", "analysis/1");
    assert!(!accepted(&refused));
    assert_eq!(field(&refused, "state"), &state);
    let invalid = replace(
        &state,
        "token_count",
        number(field(&state, "token_count").value_type(), 5),
    );
    assert!(!f.native_ok(&invalid));
    let result = f.step(&invalid, "shift", "dep", "analysis/1");
    assert!(!accepted(&result));
}

#[test]
fn proposed_arc_that_closes_a_cycle_is_refused() {
    let mut f = Fixture::new();
    let state = f.initial(2);
    let shifted = f.step(&state, "shift", "dep", "analysis/1");
    let state = field(&shifted, "state");
    let heads = field(state, "heads");
    let StructuredInfoValueShape::Collection(values) = heads.shape() else {
        panic!("heads")
    };
    let mut changed = values.to_vec();
    changed[0] = number(changed[0].value_type(), 1);
    let state = replace(
        state,
        "heads",
        StructuredInfoValue::collection(heads.value_type().clone(), changed).unwrap(),
    );
    let refused = f.step(&state, "right_arc", "obj", "analysis/1");
    assert!(!accepted(&refused));
    assert_eq!(field(&refused, "state"), &state);
}

#[test]
fn invalid_raw_graph_is_refused_by_native_and_source_admission() {
    let mut f = Fixture::new();
    let state = f.initial(2);
    assert!(f.native_ok(&state));
    let heads = field(&state, "heads");
    let StructuredInfoValueShape::Collection(values) = heads.shape() else {
        panic!("heads")
    };
    let mut changed = values.to_vec();
    changed[0] = number(changed[0].value_type(), 4);
    let invalid = replace(
        &state,
        "heads",
        StructuredInfoValue::collection(heads.value_type().clone(), changed).unwrap(),
    );
    assert!(!f.native_ok(&invalid));
    let refused = f.step(&invalid, "right_arc", "root", "analysis/1");
    assert!(!accepted(&refused));
    assert_eq!(tag(field(&refused, "refusal")), "invalid_state");
    assert_eq!(field(&refused, "state"), &invalid);
}
