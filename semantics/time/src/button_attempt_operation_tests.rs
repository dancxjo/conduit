use super::*;
use conduit_core::encode_monotonic_duration;
use conduit_kernel::ValueStorage;

fn operation(
    store: &mut conduit_kernel::HostedValueStore,
    maximum_transitions: u64,
) -> TimedButtonAttemptOperation {
    let durations = (0..maximum_transitions)
        .map(|_| store.store(&encode_monotonic_duration(50)).unwrap())
        .collect();
    TimedButtonAttemptOperation {
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        durations,
        released: Vec::with_capacity(maximum_transitions as usize + 1),
        next_duration: 0,
        next_request: 0,
        pending: None,
        cancellation: None,
        queued_transition: None,
        accepted_transitions: 0,
        maximum_transitions,
        retain_resumed: false,
        completed: false,
    }
}

#[test]
fn fired_deadline_is_a_distinct_timeout_failure() {
    let mut store = conduit_kernel::HostedValueStore::new(8, 1024, 4096).unwrap();
    let transition = store.store(b"transition").unwrap();
    let marker = store.store(&[0]).unwrap();
    let mut operation = operation(&mut store, 2);
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: transition
        }),
        OperationAction::RequestHostOperation {
            operation: HostOperationId(1),
            ..
        }
    ));
    assert!(matches!(
        operation.resume_host_operation(
            RequestId(0),
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(marker, 1).unwrap()),
                failure: None,
            },
            Some(&[0]),
        ),
        OperationAction::RequestHostOperation {
            operation: HostOperationId(0),
            ..
        }
    ));
    assert!(matches!(
        operation.resume(OperationInput::HostOperationCompleted {
            request: RequestId(1),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            }
        }),
        OperationAction::Fail(Failure {
            code: FailureCode::HostOperationFailed,
            detail: 4
        })
    ));
}

#[test]
fn closed_input_before_the_required_presses_is_not_timeout_or_exhaustion() {
    let mut store = conduit_kernel::HostedValueStore::new(8, 1024, 4096).unwrap();
    let mut operation = operation(&mut store, 2);
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 2
        })
    );
}

#[test]
fn total_transition_exhaustion_is_not_timeout_or_malformed_input() {
    let mut store = conduit_kernel::HostedValueStore::new(8, 1024, 4096).unwrap();
    let first = store.store(b"released-1").unwrap();
    let second = store.store(b"released-2").unwrap();
    let mut operation = operation(&mut store, 1);
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: first
        }),
        OperationAction::RequestHostOperation { .. }
    ));
    assert!(matches!(
        operation.resume_host_operation(
            RequestId(0),
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
            None,
        ),
        OperationAction::Await
    ));
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: second
        }),
        OperationAction::Fail(Failure {
            code: FailureCode::StorageExhausted,
            detail: 1
        })
    ));
}

#[test]
fn shared_operation_retains_transition_until_exact_deadline_cancellation() {
    let mut store = conduit_kernel::HostedValueStore::new(12, 1024, 4096).unwrap();
    let first = store.store(b"first").unwrap();
    let next = store.store(b"next").unwrap();
    let marker = store.store(&[0]).unwrap();
    let mut prepared = operation(&mut store, 3);
    let operation: &mut dyn conduit_kernel::Operation = &mut prepared;
    assert!(operation.accepts_input_while_host_operation_pending());
    assert!(matches!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: first
        }),
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            ..
        }
    ));
    assert!(matches!(
        operation.resume_host_operation(
            RequestId(0),
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(marker, 1).unwrap()),
                failure: None,
            },
            Some(&[0])
        ),
        OperationAction::RequestHostOperation {
            request: RequestId(1),
            operation: HostOperationId(0),
            ..
        }
    ));
    assert_eq!(
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: next
        }),
        OperationAction::Await
    );
    assert!(operation.retains_resumed_value());
    assert_eq!(
        operation.take_host_operation_cancellation(),
        Some(RequestId(1))
    );
    assert_eq!(operation.take_host_operation_cancellation(), None);
    assert!(
        matches!(operation.resume_host_operation(RequestId(1), HostOperationOutcome {
        disposition: HostOperationDisposition::Cancelled, output: None, failure: None,
    }, None), OperationAction::RequestHostOperation { request: RequestId(2), operation: HostOperationId(1), input } if input.value == next)
    );
}

#[test]
fn observation_request_uses_the_callers_admitted_input_bound() {
    let mut store = conduit_kernel::HostedValueStore::new(8, 4096, 32768).unwrap();
    let value = store.store(b"transition").unwrap();
    let mut operation = TimedButtonAttemptOperation::from_prepared_durations(Vec::new(), 2, 4096);
    let OperationAction::RequestHostOperation { input, .. } =
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value,
        })
    else {
        panic!("transition must request clock observation");
    };
    assert_eq!(input.admitted_bytes, 4096);
    assert_eq!(input.value, value);
}
