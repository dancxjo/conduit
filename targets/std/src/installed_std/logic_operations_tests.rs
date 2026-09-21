use super::{
    CompareOperator, DecisionValues, LogicCompareScalarOperation, LogicNotOperation,
    LogicSelectScalarOperation,
};
use conduit_core::{InfoBool, Scalar, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    HostedValueStore, PortId, ValueRef, ValueStorage,
};

fn value(slot: u16, byte_len: usize) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: byte_len as u32,
    }
}

fn store() -> HostedValueStore {
    HostedValueStore::new(2, SCALAR_ENCODED_LEN as u32, 2).expect("decision store is finite")
}

fn test_compare(operator: &str, store: &mut HostedValueStore) -> LogicCompareScalarOperation {
    LogicCompareScalarOperation {
        operator: CompareOperator::parse(operator).expect("test operator is supported"),
        operands: [None; 2],
        decisions: DecisionValues::prepare(store).expect("test decision values fit"),
    }
}

fn test_not(store: &mut HostedValueStore) -> LogicNotOperation {
    LogicNotOperation {
        received: false,
        decisions: DecisionValues::prepare(store).expect("test decision values fit"),
    }
}

fn test_select() -> LogicSelectScalarOperation {
    LogicSelectScalarOperation {
        selector: None,
        selector_closed: false,
        candidates: [None; 2],
        candidate_seen: [false; 2],
    }
}

#[test]
fn compare_implements_the_complete_finite_operator_set_at_scalar_boundaries() {
    for (operator, left, right, expected) in [
        ("lt", Scalar::MIN, Scalar::MAX, true),
        ("le", Scalar::MIN, Scalar::MIN, true),
        ("eq", Scalar::MAX, Scalar::MAX, true),
        ("ne", Scalar::MIN, Scalar::MAX, true),
        ("ge", Scalar::MAX, Scalar::MAX, true),
        ("gt", Scalar::MAX, Scalar::MIN, true),
        ("lt", Scalar::MAX, Scalar::MIN, false),
        ("le", Scalar::MAX, Scalar::MIN, false),
        ("eq", Scalar::MIN, Scalar::MAX, false),
        ("ne", Scalar::MIN, Scalar::MIN, false),
        ("ge", Scalar::MIN, Scalar::MAX, false),
        ("gt", Scalar::MIN, Scalar::MAX, false),
    ] {
        let mut values = store();
        let mut compare = test_compare(operator, &mut values);
        let left_bytes = left.encode();
        let mut io = StepIo::test_frame(
            [Some(value(10, SCALAR_ENCODED_LEN)), None],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert_eq!(
            compare.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&left_bytes), None], None)
            ),
            StepOutcome::Progress
        );
        let right_bytes = right.encode();
        let mut io = StepIo::test_frame(
            [None, Some(value(11, SCALAR_ENCODED_LEN))],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert_eq!(
            compare.step(
                &mut io,
                &StepInputBytes::test_frame([None, Some(&right_bytes)], None)
            ),
            StepOutcome::Progress
        );
        let mut io = StepIo::test_frame(
            [None; 2],
            [false; 2],
            [Some(BOOL_ENCODED_LEN as u32), None],
            None,
            8,
        );
        assert_eq!(
            compare.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
        let output = io.test_output(PortId(0)).expect("exact decision output");
        assert_eq!(
            InfoBool::decode(values.get(output).expect("decision value remains stored"))
                .expect("decision is canonical")
                .get(),
            expected
        );
        assert_eq!(io.test_discards().iter().flatten().count(), 1);
    }
}

#[test]
fn not_rejects_noncanonical_bool_and_closure_releases_both_prepared_decisions() {
    let mut values = store();
    let mut not = test_not(&mut values);
    let mut io = StepIo::test_frame(
        [Some(value(4, BOOL_ENCODED_LEN))],
        [false],
        [Some(BOOL_ENCODED_LEN as u32)],
        None,
        8,
    );
    assert!(matches!(
        not.step(&mut io, &StepInputBytes::test_frame([Some(&[2])], None)),
        StepOutcome::Fail(_)
    ));

    let mut values = store();
    let mut not = test_not(&mut values);
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        not.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_consumed_closed(PortId(0)));
    assert_eq!(io.test_discards().iter().flatten().count(), 2);
}

#[test]
fn select_retains_unknown_candidates_then_transfers_only_the_selected_identity() {
    let mut select = test_select();
    let when_false = value(1, SCALAR_ENCODED_LEN);
    let when_true = value(2, SCALAR_ENCODED_LEN);
    let false_bytes = Scalar::MIN.encode();
    let mut io = StepIo::test_frame(
        [None, Some(when_false), None],
        [false; 3],
        [Some(SCALAR_ENCODED_LEN as u32), None, None],
        None,
        8,
    );
    assert_eq!(
        select.step(
            &mut io,
            &StepInputBytes::test_frame([None, Some(&false_bytes), None], None)
        ),
        StepOutcome::Progress
    );
    assert!(io.test_retained(PortId(1)));
    let true_bytes = Scalar::MAX.encode();
    let mut io = StepIo::test_frame(
        [None, None, Some(when_true)],
        [false; 3],
        [Some(SCALAR_ENCODED_LEN as u32), None, None],
        None,
        8,
    );
    assert_eq!(
        select.step(
            &mut io,
            &StepInputBytes::test_frame([None, None, Some(&true_bytes)], None)
        ),
        StepOutcome::Progress
    );
    assert!(io.test_retained(PortId(2)));
    let selector_bytes = InfoBool::TRUE.encode();
    let mut io = StepIo::test_frame(
        [Some(value(3, BOOL_ENCODED_LEN)), None, None],
        [false; 3],
        [Some(SCALAR_ENCODED_LEN as u32), None, None],
        None,
        8,
    );
    assert_eq!(
        select.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&selector_bytes), None, None], None)
        ),
        StepOutcome::Progress
    );
    let mut io = StepIo::test_frame(
        [None; 3],
        [false; 3],
        [Some(SCALAR_ENCODED_LEN as u32), None, None],
        None,
        8,
    );
    assert_eq!(
        select.step(&mut io, &StepInputBytes::test_frame([None; 3], None)),
        StepOutcome::Complete
    );
    assert_eq!(io.test_output(PortId(0)), Some(when_true));
    assert!(io.test_discards().contains(&Some(when_false)));
}

#[test]
fn select_unknown_selector_closure_releases_both_retained_candidates_atomically() {
    let mut select = test_select();
    let when_false = value(1, SCALAR_ENCODED_LEN);
    let when_true = value(2, SCALAR_ENCODED_LEN);
    for (port, candidate, bytes) in [
        (PortId(1), when_false, Scalar::MIN.encode()),
        (PortId(2), when_true, Scalar::MAX.encode()),
    ] {
        let mut inputs = [None; 3];
        let mut canonical = [None; 3];
        inputs[usize::from(port.0)] = Some(candidate);
        canonical[usize::from(port.0)] = Some(bytes.as_slice());
        let mut io = StepIo::test_frame(inputs, [false; 3], [None; 3], None, 8);
        assert_eq!(
            select.step(&mut io, &StepInputBytes::test_frame(canonical, None)),
            StepOutcome::Progress
        );
    }
    let mut io = StepIo::test_frame([None; 3], [true, false, false], [None; 3], None, 8);
    assert_eq!(
        select.step(&mut io, &StepInputBytes::test_frame([None; 3], None)),
        StepOutcome::Progress
    );
    let mut io = StepIo::test_frame([None; 3], [false; 3], [None; 3], None, 8);
    assert_eq!(
        select.step(&mut io, &StepInputBytes::test_frame([None; 3], None)),
        StepOutcome::Complete
    );
    assert!(io.test_discards().contains(&Some(when_false)));
    assert!(io.test_discards().contains(&Some(when_true)));
}

#[test]
fn cancellation_clears_all_operation_owned_decision_state() {
    let mut select = test_select();
    let scalar = Scalar::MIN.encode();
    let mut io = StepIo::test_frame(
        [None, Some(value(1, SCALAR_ENCODED_LEN)), None],
        [false; 3],
        [None; 3],
        None,
        8,
    );
    assert_eq!(
        select.step(
            &mut io,
            &StepInputBytes::test_frame([None, Some(&scalar), None], None)
        ),
        StepOutcome::Progress
    );
    assert!(io.test_retained(PortId(1)));
    StepOperation::<3>::cancel(&mut select);
    assert_eq!(select.candidates, [None; 2]);

    let mut values = store();
    let mut compare = test_compare("eq", &mut values);
    let zero = Scalar::ZERO.encode();
    let mut io = StepIo::test_frame(
        [Some(value(2, SCALAR_ENCODED_LEN)), None],
        [false; 2],
        [None; 2],
        None,
        8,
    );
    assert_eq!(
        compare.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&zero), None], None)
        ),
        StepOutcome::Progress
    );
    StepOperation::<2>::cancel(&mut compare);
    assert_eq!(compare.operands, [None; 2]);
}
