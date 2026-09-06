use super::*;

#[test]
fn capacity_work_and_identity_refusals_preserve_the_last_committed_state() {
    let capacity = StateDelay::<2>::externally_continued(0, 1, &[0]).unwrap();
    let mut work = StateDelay::<2>::new(0, 1, 1, &[0]).unwrap();
    work.commit().unwrap();
    let mut identity = StateDelay::<2>::externally_continued(0, 1, &[0]).unwrap();
    // Exercise the exact finite identity boundary without enumerating generations.
    identity.generation = u64::MAX;
    for (state, next, code, detail) in [
        (
            capacity,
            &[1, 2][..],
            FailureCode::StateCapacityExhausted,
            1,
        ),
        (work, &[1][..], FailureCode::WorkBudgetExhausted, 2),
        (
            identity,
            &[1][..],
            FailureCode::IdentityCapacityExhausted,
            3,
        ),
    ] {
        let generation = state.generation();
        let mut operation = StateOperation::new(state, PortId(0), PortId(0)).unwrap();
        assert!(matches!(
            operation.start(),
            OperationAction::EmitCanonical { .. }
        ));
        operation.step_committed();
        assert_eq!(
            operation.resume_value(
                PortId(0),
                ValueRef {
                    slot: 0,
                    generation: 0,
                    byte_len: next.len() as u32
                },
                next,
            ),
            OperationAction::Fail(Failure { code, detail }),
        );
        assert!(operation.is_terminal());
        assert_eq!(operation.state().current(), &[0]);
        assert_eq!(operation.state().generation(), generation);
        assert!(operation.state.candidate_len.is_none());
    }
}
