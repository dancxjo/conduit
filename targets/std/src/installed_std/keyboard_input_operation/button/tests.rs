use super::*;

fn value(slot: u16, byte_len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len,
    }
}

fn source() -> ButtonOperation {
    ButtonOperation {
        empty: value(0, 0),
        empty_released: false,
        next: 0,
        pending: None,
        terminal: false,
        validator: PreparedStructuredValueValidator::new(
            &conduit_semantic_catalog::input_button_transition_type(),
            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES as usize,
        )
        .unwrap(),
    }
}

fn transition(pressed: bool, sequence: u64) -> Vec<u8> {
    conduit_semantic_catalog::button_transition_value("button/primary", pressed, sequence)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn completed(output: ValueRef) -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: Some(
            BoundedValueRef::new(
                output,
                conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
            )
            .unwrap(),
        ),
        failure: None,
    }
}

fn request(action: OperationAction, expected: u32) {
    assert!(matches!(
        action,
        OperationAction::RequestHostOperation {
            request: RequestId(id),
            operation: HostOperationId(0),
            ..
        } if id == expected
    ));
}

#[test]
fn transitions_continue_in_one_play_until_explicit_stop() {
    let mut source = source();
    request(source.start(), 0);
    for (index, pressed) in [true, false, true, false, true, false]
        .into_iter()
        .enumerate()
    {
        let canonical = transition(pressed, index as u64);
        let output = value(2 + index as u16, canonical.len() as u32);
        assert_eq!(
            source.resume_host_operation(
                RequestId(index as u32),
                completed(output),
                Some(&canonical),
            ),
            OperationAction::Emit {
                port: PortId(0),
                value: output,
            }
        );
        request(source.advance(), index as u32 + 1);
    }
    source.cancel();
    assert_eq!(source.take_released_value(), Some(value(0, 0)));
}

#[test]
fn malformed_failure_and_late_completion_remain_distinct() {
    let mut source = source();
    request(source.start(), 0);
    assert_eq!(
        source.resume_host_operation(RequestId(0), completed(value(2, 3)), Some(b"bad")),
        fail(FailureCode::InvalidInput, 5)
    );

    let mut source = self::source();
    request(source.start(), 0);
    let failure = Failure {
        code: FailureCode::HostOperationFailed,
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
            None,
        ),
        OperationAction::Fail(failure)
    );
    source.cancel();
    assert_eq!(
        source.resume_host_operation(RequestId(0), completed(value(3, 3)), Some(b"bad")),
        fail(FailureCode::InvalidLifecycle, 3)
    );
}
