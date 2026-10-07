use conduit_language::{parser_window8::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
fn basis() -> LanguageParserBasis {
    LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/analysis".into()).unwrap(),
        LanguageTextRevisionId::new("window8/r0".into()).unwrap(),
        LanguageTextId::new("window8/text".into()).unwrap(),
    )
    .unwrap()
}
fn relation(root: bool) -> LanguageParserRelation {
    LanguageParserRelation::new(
        if root {
            LanguageUniversalDependencyRelation::Root
        } else {
            LanguageUniversalDependencyRelation::Dep
        },
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap()
}
fn state(
    heads: [u64; 9],
    stack: [u64; 9],
    unread: u64,
    depth: u64,
    committed: u64,
    count: u64,
) -> LanguageParserWindow8RawState {
    LanguageParserWindow8RawState::new(
        basis(),
        committed,
        depth,
        heads,
        relation(heads[0] == 8),
        relation(heads[1] == 8),
        relation(heads[2] == 8),
        relation(heads[3] == 8),
        relation(heads[4] == 8),
        relation(heads[5] == 8),
        relation(heads[6] == 8),
        relation(heads[7] == 8),
        stack,
        count,
        unread,
    )
    .unwrap()
}
fn chain() -> LanguageParserWindow8RawState {
    state(
        [8, 0, 1, 2, 3, 4, 5, 6, 8],
        [8, 0, 1, 2, 3, 4, 5, 6, 7],
        8,
        9,
        0,
        8,
    )
}
#[test]
fn native_eight_hop_forest_proof_and_shape_frontier_laws() {
    let raw = chain();
    let admitted = prepare_window8_state(&raw).unwrap();
    assert_eq!(admitted.state(), &raw);
    assert_eq!(
        admitted.proof().ancestry()[7].path(),
        &[7, 6, 5, 4, 3, 2, 1, 0, 8]
    );
    assert_eq!(*admitted.proof().roots(), 1);
    let encoded = admitted.proof().clone().encode().unwrap();
    assert!(encoded.len() <= conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    assert_eq!(
        LanguageParserWindow8StateProof::decode(&encoded).unwrap(),
        *admitted.proof()
    );
    for invalid in [
        state(
            [1, 2, 3, 4, 5, 6, 7, 0, 8],
            [8, 0, 1, 2, 3, 4, 5, 6, 7],
            8,
            9,
            0,
            8,
        ),
        state(
            [8, 0, 1, 3, 3, 4, 5, 6, 8],
            [8, 0, 1, 2, 3, 4, 5, 6, 7],
            8,
            9,
            0,
            8,
        ),
        state(
            [8, 8, 1, 2, 3, 4, 5, 6, 8],
            [8, 0, 1, 2, 3, 4, 5, 6, 7],
            8,
            9,
            0,
            8,
        ),
        state(
            [8, 9, 9, 9, 9, 9, 9, 9, 8],
            [8, 0, 8, 8, 8, 8, 8, 8, 8],
            1,
            2,
            2,
            8,
        ),
        state(
            [8, 0, 9, 9, 9, 9, 9, 9, 8],
            [8, 0, 8, 8, 8, 8, 8, 8, 8],
            1,
            2,
            0,
            8,
        ),
        state(
            [8, 0, 1, 2, 3, 4, 5, 6, 8],
            [8, 0, 1, 2, 3, 4, 6, 5, 7],
            8,
            9,
            0,
            8,
        ),
        state(
            [8, 0, 1, 2, 3, 4, 5, 6, 8],
            [8, 0, 1, 2, 3, 4, 5, 6, 7],
            u64::MAX,
            9,
            0,
            8,
        ),
    ] {
        assert!(prepare_window8_state(&invalid).is_err());
    }
}
#[test]
fn forged_or_foreign_native_witnesses_are_not_boolean_authority() {
    let raw = chain();
    let proof = prepare_window8_state(&raw).unwrap();
    let mut path = *proof.proof().ancestry()[7].path();
    path[4] = 8;
    assert!(LanguageParserWindow8Ancestry::new(*raw.heads(), path, 7).is_err());
    let mut witnesses = proof.proof().ancestry().clone();
    witnesses.swap(0, 1);
    assert!(LanguageParserWindow8StateProof::new(witnesses, 1, raw.clone()).is_err());
    let foreign = state(
        [8, 0, 0, 2, 3, 4, 5, 6, 8],
        [8, 0, 1, 2, 3, 4, 5, 6, 7],
        8,
        9,
        0,
        8,
    );
    assert!(
        LanguageParserWindow8StateProof::new(proof.proof().ancestry().clone(), 1, foreign).is_err()
    );
    assert!(
        LanguageParserWindow8StateProof::new(proof.proof().ancestry().clone(), 0, raw).is_err()
    );
}
#[test]
fn source_initialization_keeps_root8_unassigned9_and_old_profile_disjoint() {
    let begin = LanguageParserWindow8Begin::new(basis(), relation(false), 8).unwrap();
    let prepared = initialize_window8(&begin).unwrap();
    assert_eq!(prepared.state().heads(), &[9, 9, 9, 9, 9, 9, 9, 9, 8]);
    assert_eq!(prepared.state().stack(), &[8; 9]);
    assert_eq!(*prepared.state().unread(), 0);
    assert_ne!(
        LanguageParserWindow8RawState::semantic_type().unwrap(),
        LanguageParserNumericState::semantic_type().unwrap()
    );
}

#[test]
fn source_steps_close_eight_tokens_and_preserve_refused_snapshots() {
    let begin = LanguageParserWindow8Begin::new(basis(), relation(false), 8).unwrap();
    let initial = initialize_window8(&begin).unwrap();
    let refused = prepare_window8_step(
        &initial,
        &basis(),
        LanguageParserAction::Reduce,
        &relation(false),
    )
    .unwrap();
    assert!(!refused.accepted());
    assert_eq!(refused.next().proof(), initial.proof());
    let mut current = initial;
    for ordinal in 0..8 {
        let step = prepare_window8_step(
            &current,
            &basis(),
            LanguageParserAction::RightArc,
            &relation(ordinal == 0),
        )
        .unwrap();
        assert!(step.accepted());
        assert_eq!(*step.next().state().unread(), ordinal + 1);
        current = step.next().clone();
    }
    for _ in 0..8 {
        let step = prepare_window8_step(
            &current,
            &basis(),
            LanguageParserAction::Reduce,
            &relation(false),
        )
        .unwrap();
        assert!(step.accepted());
        current = step.next().clone();
    }
    assert!(window8_complete(&current).unwrap());
    let foreign = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("foreign/analysis".into()).unwrap(),
        basis().source_revision().clone(),
        basis().text().clone(),
    )
    .unwrap();
    let stale = prepare_window8_step(
        &current,
        &foreign,
        LanguageParserAction::Shift,
        &relation(false),
    )
    .unwrap();
    assert!(!stale.accepted());
    assert_eq!(
        *stale.proposal().refusal(),
        LanguageParserRefusal::StaleBasis
    );
    assert_eq!(stale.next().proof(), current.proof());
}

#[test]
fn source_right_arc_cycle_refusal_retains_exact_prior() {
    let raw = state(
        [1, 9, 9, 9, 9, 9, 9, 9, 8],
        [8, 0, 8, 8, 8, 8, 8, 8, 8],
        1,
        2,
        0,
        8,
    );
    let prior = prepare_window8_state(&raw).unwrap();
    let refused = prepare_window8_step(
        &prior,
        &basis(),
        LanguageParserAction::RightArc,
        &relation(false),
    )
    .unwrap();
    assert!(!refused.accepted());
    assert_eq!(*refused.proposal().refusal(), LanguageParserRefusal::Cycle);
    assert_eq!(refused.next().proof(), prior.proof());
}

#[test]
fn raw_request_requires_the_exact_prior_ancestry_and_stack_top() {
    let prior =
        initialize_window8(&LanguageParserWindow8Begin::new(basis(), relation(false), 8).unwrap())
            .unwrap();
    assert!(LanguageParserWindow8RawRequest::new(
        LanguageParserAction::Shift,
        basis(),
        relation(false),
        prior.proof().clone(),
        prior.proof().ancestry()[1].clone()
    )
    .is_err());
    let changed = prepare_window8_step(
        &prior,
        &basis(),
        LanguageParserAction::RightArc,
        &relation(true),
    )
    .unwrap();
    assert!(LanguageParserWindow8RawRequest::new(
        LanguageParserAction::Shift,
        basis(),
        relation(false),
        prior.proof().clone(),
        changed.next().proof().ancestry()[0].clone()
    )
    .is_err());
}
