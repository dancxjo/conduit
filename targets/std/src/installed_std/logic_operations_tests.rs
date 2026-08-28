use super::{
    CompareOperator, DecisionValues, LogicCompareScalarOperation, LogicNotOperation,
    LogicSelectScalarOperation,
};
use conduit_core::{InfoBool, Scalar, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN};
use conduit_kernel::{
    HostedValueStore, OperationAction, OperationInput, PortId, ValueRef, ValueStorage,
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
        released: [None; 2],
        retain_resumed: false,
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
        assert_eq!(
            compare.resume_value(PortId(0), value(10, SCALAR_ENCODED_LEN), &left.encode()),
            OperationAction::Await
        );
        let output =
            match compare.resume_value(PortId(1), value(11, SCALAR_ENCODED_LEN), &right.encode()) {
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                } => value,
                action => panic!("compare did not emit its exact decision: {action:?}"),
            };
        assert_eq!(
            InfoBool::decode(values.get(output).expect("decision value remains stored"))
                .expect("decision is canonical")
                .get(),
            expected
        );
        assert!(compare.take_released_value().is_some());
        assert_eq!(compare.take_released_value(), None);
    }
}

#[test]
fn not_rejects_noncanonical_bool_and_closure_releases_both_prepared_decisions() {
    let mut values = store();
    let mut not = test_not(&mut values);
    assert!(matches!(
        not.resume_value(PortId(0), value(4, BOOL_ENCODED_LEN), &[2]),
        OperationAction::Fail(_)
    ));

    let mut values = store();
    let mut not = test_not(&mut values);
    assert_eq!(
        not.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
    assert!(not.take_released_value().is_some());
    assert!(not.take_released_value().is_some());
    assert_eq!(not.take_released_value(), None);
}

#[test]
fn select_retains_unknown_candidates_then_transfers_only_the_selected_identity() {
    let mut select = test_select();
    let when_false = value(1, SCALAR_ENCODED_LEN);
    let when_true = value(2, SCALAR_ENCODED_LEN);
    assert_eq!(
        select.resume_value(PortId(1), when_false, &Scalar::MIN.encode()),
        OperationAction::Await
    );
    assert!(select.retains_resumed_value());
    assert_eq!(
        select.resume_value(PortId(2), when_true, &Scalar::MAX.encode()),
        OperationAction::Await
    );
    assert!(select.retains_resumed_value());
    assert_eq!(
        select.resume_value(
            PortId(0),
            value(3, BOOL_ENCODED_LEN),
            &InfoBool::TRUE.encode()
        ),
        OperationAction::Emit {
            port: PortId(0),
            value: when_true,
        }
    );
    assert!(!select.retains_resumed_value());
    assert_eq!(select.take_released_value(), Some(when_false));
    assert_eq!(select.take_released_value(), None);
}

#[test]
fn select_unknown_selector_closure_releases_both_retained_candidates_atomically() {
    let mut select = test_select();
    let when_false = value(1, SCALAR_ENCODED_LEN);
    let when_true = value(2, SCALAR_ENCODED_LEN);
    select.resume_value(PortId(1), when_false, &Scalar::MIN.encode());
    select.resume_value(PortId(2), when_true, &Scalar::MAX.encode());
    assert_eq!(
        select.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
    assert_eq!(select.take_released_value(), Some(when_false));
    assert_eq!(select.take_released_value(), Some(when_true));
    assert_eq!(select.take_released_value(), None);
}

#[test]
fn cancellation_clears_all_operation_owned_decision_state() {
    let mut select = test_select();
    select.resume_value(
        PortId(1),
        value(1, SCALAR_ENCODED_LEN),
        &Scalar::MIN.encode(),
    );
    assert!(select.retains_resumed_value());
    select.cancel();
    assert!(!select.retains_resumed_value());
    assert_eq!(select.take_released_value(), None);

    let mut values = store();
    let mut compare = test_compare("eq", &mut values);
    compare.resume_value(
        PortId(0),
        value(2, SCALAR_ENCODED_LEN),
        &Scalar::ZERO.encode(),
    );
    compare.cancel();
    assert_eq!(compare.take_released_value(), None);
}
