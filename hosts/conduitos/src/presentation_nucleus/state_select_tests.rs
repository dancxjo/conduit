use alloc::vec::Vec;
use conduit_core::{InfoBool, Scalar};
use conduit_kernel::{
    Failure, FailureCode, Operation, OperationAction, OperationInput, PortId, ValueRef,
};

use super::{
    StateSelectSequence, prepare_state_select, run_state_select,
    state_select_operation::StateSelectOperation, state_select_play::cancel_state_select,
};

fn scalar(raw: i64) -> Scalar {
    Scalar::from_raw_microunits(raw)
}

fn emitted(action: OperationAction) -> Option<Vec<u8>> {
    match action {
        OperationAction::EmitCanonical { value, .. } => Some(value.as_slice().to_vec()),
        _ => None,
    }
}

fn value(slot: u16, byte_len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len,
    }
}

#[test]
fn fixed_kernel_selects_false_and_true_with_exact_profile_identity() {
    for (selector, expected) in [(InfoBool::FALSE, scalar(10)), (InfoBool::TRUE, scalar(20))] {
        let prepared = prepare_state_select(
            "select-host",
            "select-boot",
            StateSelectSequence::one(selector, scalar(10), scalar(20)),
        )
        .unwrap();
        let placement = prepared.plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::STATE_SELECT_KIND)
            .unwrap();
        assert_eq!(
            placement.implementation_id.as_str(),
            conduit_std_catalog::CONDUITOS_STATE_SELECT_SCALAR_IMPLEMENTATION
        );
        let proof = run_state_select(&prepared).unwrap();
        assert_eq!(proof.outputs[0], Some(expected));
        assert_eq!(proof.output_count, 1);
        assert_eq!(proof.maximum_cord_items, 1);
        assert!(proof.decisions > 0 && proof.kernel_signs > 0);
    }
}

#[test]
fn fixed_kernel_preserves_selector_and_candidate_updates_under_capacity_one_pressure() {
    let selector_change = prepare_state_select(
        "select-host",
        "select-boot",
        StateSelectSequence {
            selectors: [Some(InfoBool::FALSE), Some(InfoBool::TRUE)],
            when_false: [Some(scalar(10)), None],
            when_true: [Some(scalar(20)), None],
        },
    )
    .unwrap();
    let proof = run_state_select(&selector_change).unwrap();
    let outputs = proof.outputs[..proof.output_count]
        .iter()
        .copied()
        .flatten()
        .collect::<Vec<_>>();
    assert!(outputs.contains(&scalar(10)), "{outputs:?}");
    assert_eq!(outputs.last(), Some(&scalar(20)), "{outputs:?}");
    assert_eq!(proof.maximum_cord_items, 1);

    let candidate_change = prepare_state_select(
        "select-host",
        "select-boot",
        StateSelectSequence {
            selectors: [Some(InfoBool::FALSE), None],
            when_false: [Some(scalar(10)), Some(scalar(11))],
            when_true: [Some(scalar(20)), None],
        },
    )
    .unwrap();
    let proof = run_state_select(&candidate_change).unwrap();
    let outputs = proof.outputs[..proof.output_count]
        .iter()
        .copied()
        .flatten()
        .collect::<Vec<_>>();
    assert!(outputs.contains(&scalar(10)), "{outputs:?}");
    assert_eq!(outputs.last(), Some(&scalar(11)), "{outputs:?}");
    assert_eq!(proof.maximum_cord_items, 1);
}

#[test]
fn malformed_closed_and_cancelled_selector_inputs_fail_without_state_fabrication() {
    let mut operation = StateSelectOperation::Select {
        selector: None,
        candidates: [None; 2],
        closed: [false; 3],
    };
    assert_eq!(
        operation.resume_value(PortId(0), value(0, 1), &[2]),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 72,
        })
    );
    assert_eq!(
        operation.resume_value(PortId(1), value(0, 8), &[0; 7]),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 72,
        })
    );

    let selector = InfoBool::FALSE.encode();
    let false_value = scalar(10).encode();
    let true_value = scalar(20).encode();
    assert_eq!(
        operation.resume_value(PortId(0), value(1, 1), &selector),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume_value(PortId(1), value(2, 8), &false_value),
        OperationAction::Await
    );
    assert_eq!(
        emitted(operation.resume_value(PortId(2), value(3, 8), &true_value)),
        Some(false_value.to_vec())
    );
    operation.cancel();
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(2) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume_value(PortId(2), value(4, 8), &true_value),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 72,
        })
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(1) }),
        OperationAction::Complete
    );

    let prepared = prepare_state_select(
        "select-host",
        "select-boot",
        StateSelectSequence::one(InfoBool::FALSE, scalar(10), scalar(20)),
    )
    .unwrap();
    assert_eq!(cancel_state_select(&prepared).unwrap(), [true, true]);
}
