use super::*;

#[test]
fn declared_read_cancellation_preserves_kernel_outcome_and_append_progress_under_pressure() {
    const BYTES: usize = crate::read_result::READ_RESULT_BYTES;
    let mut selected = shape();
    selected.read.maximum_output_bytes = BYTES as u32;
    let back = RecordServiceBack::new_with_declared_read_cancellation(selected).unwrap();
    let mut f = Fixture::<BYTES>::with_back(back);
    f.input(1);
    let old = f.request(1, 0);
    let mut encoder = crate::read_result::Encoder::new().unwrap();
    let previous = encoder
        .encode(crate::read_result::ReadResult::End(0))
        .unwrap()
        .to_vec();
    f.complete(old, &previous);
    f.step();
    f.step();
    f.input(1);
    let cancelled = f.request(1, 1);
    let outcome = conduit_kernel::HostCallOutcome {
        disposition: HostCallDisposition::Cancelled,
        output: None,
        failure: None,
    };
    f.kernel
        .complete_host_call(cancelled.node, cancelled.request, outcome)
        .unwrap();
    // Capture only becomes visible in private staging after actual commit.
    f.reject.set(true);
    assert_eq!(f.kernel.step(), Err(SchedulerError::FalseProgress));
    assert_eq!(f.view.get().read_len, None);
    assert_eq!(f.view.get().pending, Some((RequestId(1), 1)));
    f.step();
    assert_eq!(f.kernel.pending_host_call_count(), 0);
    assert_eq!(
        f.view.get().read_len,
        Some(crate::read_result::cancelled().len())
    );
    assert_eq!(f.output(1).unwrap(), previous);
    f.input(1); // Cannot overwrite the undelivered cancellation result.
    for id in 2..4 {
        f.input(0);
        let request = f.request(0, id);
        f.complete(request, b"appended");
        f.step();
        assert_eq!(f.output(0).unwrap(), b"appended");
        f.drain(0);
        assert_eq!(f.output(1).unwrap(), previous);
        assert_eq!(
            f.view.get().read_len,
            Some(crate::read_result::cancelled().len())
        );
    }
    // The actual cancelled request cannot complete again after consumption.
    assert_eq!(
        f.kernel
            .complete_host_call(cancelled.node, cancelled.request, outcome),
        Err(SchedulerError::HostCallCompletionRejected)
    );
    f.drain(1);
    f.step();
    let emitted = f.output(1).unwrap();
    assert_eq!(emitted, crate::read_result::cancelled());
    assert_eq!(
        crate::read_result::decode(&emitted).unwrap(),
        crate::read_result::ReadResult::Cancelled
    );
    let next = f.request(1, 4);
    assert_eq!(next.request, RequestId(4));
}
#[test]
fn undeclared_read_or_append_cancellation_remains_terminal() {
    for operation in 0..2 {
        let mut f = Fixture::<8>::new();
        f.input(operation);
        let request = f.request(operation, 0);
        f.kernel
            .complete_host_call(
                request.node,
                request.request,
                conduit_kernel::HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                },
            )
            .unwrap();
        assert!(matches!(
            f.kernel.step(),
            Err(SchedulerError::BackFailed(Failure {
                code: FailureCode::Cancelled,
                ..
            }))
        ));
    }
    // Opting in for READ must not change APPEND terminal semantics.
    const BYTES: usize = crate::read_result::READ_RESULT_BYTES;
    let mut selected = shape();
    selected.read.maximum_output_bytes = BYTES as u32;
    let mut f = Fixture::<BYTES>::with_back(
        RecordServiceBack::new_with_declared_read_cancellation(selected).unwrap(),
    );
    f.input(0);
    let request = f.request(0, 0);
    f.kernel
        .complete_host_call(
            request.node,
            request.request,
            conduit_kernel::HostCallOutcome {
                disposition: HostCallDisposition::Cancelled,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    assert!(matches!(
        f.kernel.step(),
        Err(SchedulerError::BackFailed(Failure {
            code: FailureCode::Cancelled,
            ..
        }))
    ));
    assert_eq!(f.view.get().read_len, None);
    assert!(matches!(
        RecordServiceBack::<2, 8>::new_with_declared_read_cancellation(shape()),
        Err(RecordServiceBackRefusal::InvalidDeclaredReadCancellation)
    ));
}
