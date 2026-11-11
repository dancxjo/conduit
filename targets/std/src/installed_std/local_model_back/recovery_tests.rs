use super::*;
use conduit_core::{TerminalCategory, TerminalInfo};
use conduit_kernel::{scheduler::CanonicalValue, HostCallOutcome};
fn operation(recoverable: bool) -> LocalModelBack {
    LocalModelBack {
        maximum_input_bytes: 4096,
        provider_loss_terminal: recoverable.then(|| {
            CanonicalValue::new(
                &TerminalInfo::new(TerminalCategory::ProviderOrResourceLoss, [3; 32]).encode(),
            )
            .unwrap()
        }),
        loss_staged: false,
        pending: Some(RequestId(7)),
        next_request: 8,
        closed: false,
        flow: true,
        emitted: false,
        stream: false,
        stream_complete: false,
        input: None,
    }
}
fn completion(request: RequestId, detail: u16) -> StepIo<1> {
    StepIo::test_frame(
        [None],
        [false],
        [None],
        Some((
            request,
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(Failure {
                    code: if detail == 55 {
                        FailureCode::InvalidInput
                    } else {
                        FailureCode::HostCallFailed
                    },
                    detail,
                }),
            },
        )),
        8,
    )
}
#[test]
fn actual_loss_emits_prepared_abnormal_without_a_result_or_early_commit() {
    let mut back = operation(true);
    let mut io = completion(RequestId(7), 54);
    let StepOutcome::Abnormal { port, terminal } =
        back.step(&mut io, &StepInputBytes::test_frame([None], None))
    else {
        panic!("provider loss is typed abnormal");
    };
    assert_eq!(port, PortId(0));
    assert_eq!(
        TerminalInfo::decode(terminal.as_slice())
            .unwrap()
            .category(),
        TerminalCategory::ProviderOrResourceLoss
    );
    assert!(io.test_host_completion_consumed());
    assert!(io.test_output(PortId(0)).is_none());
    assert_eq!(back.pending, Some(RequestId(7)));
    assert!(!back.closed);
    StepBack::<1>::step_committed(&mut back);
    assert!(back.pending.is_none());
    assert!(back.closed);
}
#[test]
fn mechanism_faults_and_original_contract_never_become_recovered_loss() {
    for (recoverable, detail) in [(true, 53), (true, 55), (false, 54)] {
        let mut back = operation(recoverable);
        let mut io = completion(RequestId(7), detail);
        assert!(
            matches!(back.step(&mut io, &StepInputBytes::test_frame([None], None)), StepOutcome::Fail(Failure { detail: actual, .. }) if actual == detail)
        );
        assert!(!io.test_host_completion_consumed());
        assert!(!back.loss_staged);
    }
    let mut back = operation(true);
    let mut io = completion(RequestId(6), 54);
    assert!(matches!(
        back.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
}
