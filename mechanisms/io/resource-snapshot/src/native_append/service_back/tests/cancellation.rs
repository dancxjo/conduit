use super::*;

#[test]
fn read_or_append_cancellation_remains_terminal() {
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
}
