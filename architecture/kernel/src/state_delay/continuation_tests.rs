use super::*;

#[test]
fn externally_driven_state_has_finite_storage_without_a_semantic_step_count() {
    let mut state = StateDelay::<1>::externally_continued(4, 1, &[0]).unwrap();
    let current_storage = state.current.as_ptr();
    let candidate_storage = state.candidate.as_ptr();
    assert_eq!(state.maximum_transitions, None);
    for generation in 1..=10_000 {
        let next = state.current()[0] ^ 1;
        let offered = state.offer_next(&[next]).unwrap();
        let committed = state.commit().unwrap();
        assert_eq!(offered.candidate, committed.candidate);
        assert_eq!(committed.current.generation, generation);
        assert_eq!(committed.candidate.unwrap().generation, generation);
        assert_eq!(state.current(), &[next]);
        assert_eq!(state.current.as_ptr(), current_storage);
        assert_eq!(state.candidate.as_ptr(), candidate_storage);
    }
    let generation = state.generation();
    assert_eq!(state.current(), &[0]);
    assert_eq!(state.generation(), generation);
    assert_eq!(state.maximum_transitions, None);
}

#[test]
fn generation_capacity_refuses_without_wrap_or_silent_reset() {
    let mut state = StateDelay::<1>::externally_continued(1, 1, &[0]).unwrap();
    state.generation = u64::MAX - 1;
    let offered = state.offer_next(&[1]).unwrap();
    assert_eq!(offered.candidate.unwrap().generation, u64::MAX);
    let committed = state.commit().unwrap();
    assert_eq!(committed.candidate, offered.candidate);
    assert_eq!(committed.current.generation, u64::MAX);
    assert_eq!(
        state.offer_next(&[0]),
        Err(StateError::IdentityCapacityExhausted)
    );
    assert_eq!(state.commit(), Err(StateError::IdentityCapacityExhausted));
    assert_eq!(state.current(), &[1]);
    assert_eq!(state.generation(), u64::MAX);
    assert_eq!(state.candidate_len, None);
}

#[test]
fn cancellation_discards_candidate_without_renewing_or_relabeling_it() {
    let mut state = StateDelay::<1>::externally_continued(1, 1, &[0]).unwrap();
    let offered = state.offer_next(&[1]).unwrap();
    let cancelled = state.abort_step(true);
    assert_eq!(cancelled.candidate, offered.candidate);
    assert_eq!(cancelled.transition, StateTransition::Cancelled);
    assert_eq!(state.current(), &[0]);
    assert_eq!(state.generation(), 0);
    assert_eq!(
        state.commit().unwrap().transition,
        StateTransition::HeldWithoutCandidate
    );
    assert_eq!(state.generation(), 1);
}

#[test]
fn finite_transition_budget_still_exhausts_without_committing_candidate() {
    let mut state = StateDelay::<1>::new(1, 1, 1, &[0]).unwrap();
    state.commit().unwrap();
    let candidate = state.offer_next(&[1]).unwrap();
    assert_eq!(state.commit(), Err(StateError::TransitionLimitReached));
    assert_eq!(state.current(), &[0]);
    assert_eq!(state.generation(), 1);
    assert_eq!(state.abort_step(false).candidate, candidate.candidate);
    assert!(matches!(
        StateDelay::<1>::new(1, 1, 0, &[0]),
        Err(StateError::InvalidBounds)
    ));
}
