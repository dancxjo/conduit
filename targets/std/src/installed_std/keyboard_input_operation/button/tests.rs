use super::*;

fn value(slot: u16) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: 0,
    }
}
fn source() -> ButtonOperation {
    ButtonOperation {
        empty: value(0),
        transitions: vec![value(2), value(3)],
        empty_released: false,
        emitted: 0,
        next: 0,
        pending: None,
        held: false,
        terminal: false,
    }
}
fn completed() -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: Some(BoundedValueRef::new(value(5), 3).unwrap()),
        failure: None,
    }
}
fn request(action: OperationAction, expected: u32) {
    assert!(
        matches!(action, OperationAction::RequestHostOperation { request: RequestId(id), operation: HostOperationId(0), .. } if id == expected)
    );
}

#[test]
fn space_press_release_emits_admitted_transitions_then_closes() {
    let mut source = source();
    request(source.start(), 0);
    assert_eq!(
        source.resume_host_operation(RequestId(0), completed(), Some(&[0x2c, 0, 0])),
        OperationAction::Emit {
            port: PortId(0),
            value: value(2)
        }
    );
    request(source.advance(), 1);
    assert_eq!(
        source.resume_host_operation(RequestId(1), completed(), Some(&[0x2c, 1, 0])),
        OperationAction::Emit {
            port: PortId(0),
            value: value(3)
        }
    );
    assert_eq!(source.advance(), OperationAction::Complete);
    assert_eq!(source.advance(), fail(FailureCode::InvalidLifecycle, 1));
}

#[test]
fn unrelated_keys_consume_finite_request_budget_not_semantic_sequence() {
    let mut source = source();
    request(source.start(), 0);
    for id in 0..super::super::MAX_PLAY_EVENTS {
        let action = source.resume_host_operation(RequestId(id), completed(), Some(&[4, 0, 0]));
        if id + 1 == super::super::MAX_PLAY_EVENTS {
            assert_eq!(action, fail(FailureCode::StorageExhausted, 2));
        } else {
            request(action, id + 1);
        }
    }
    assert_eq!(source.emitted, 0);
}

#[test]
fn stale_completion_malformed_repeat_and_cancel_are_not_transitions() {
    let mut source = source();
    request(source.start(), 0);
    assert_eq!(
        source.resume_host_operation(RequestId(9), completed(), Some(&[0x2c, 0, 0])),
        fail(FailureCode::InvalidLifecycle, 3)
    );
    assert_eq!(source.pending, Some(RequestId(0)));
    assert!(matches!(
        source.resume_host_operation(RequestId(0), completed(), Some(&[0x2c, 0, 0])),
        OperationAction::Emit { .. }
    ));
    request(source.advance(), 1);
    assert_eq!(
        source.resume_host_operation(RequestId(1), completed(), Some(&[0x2c, 0, 0])),
        fail(FailureCode::InvalidInput, 5)
    );
    assert_eq!(source.emitted, 1);

    let mut source = self::source();
    request(source.start(), 0);
    assert_eq!(
        source.resume_host_operation(RequestId(0), completed(), Some(b"bad-key")),
        fail(FailureCode::InvalidInput, 4)
    );
    assert_eq!(source.emitted, 0);
    source.cancel();
    assert_eq!(
        source.resume_host_operation(RequestId(0), completed(), Some(&[0x2c, 0, 0])),
        fail(FailureCode::InvalidLifecycle, 3)
    );
}

#[test]
fn unmatched_release_and_host_failure_are_explicit() {
    let mut source = source();
    request(source.start(), 0);
    assert_eq!(
        source.resume_host_operation(RequestId(0), completed(), Some(&[0x2c, 1, 0])),
        fail(FailureCode::InvalidInput, 5)
    );
    let mut source = self::source();
    request(source.start(), 0);
    let failure = Failure {
        code: FailureCode::InvalidInput,
        detail: 42,
    };
    assert_eq!(
        source.resume_host_operation(
            RequestId(0),
            HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                failure: Some(failure),
                output: None,
            },
            None
        ),
        OperationAction::Fail(failure)
    );
}
