//! Exact optional sound sink, shared by native ConduitOS and hosted profiles.
//! Optionality belongs to `sound/startup-chime`: the kernel still retains the
//! actual Host Call completion/denial/failure before this sink consumes it.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};
#[derive(Default)]
pub struct StartupChimeBack {
    pending: Option<RequestId>,
    next: u32,
}
impl StartupChimeBack {
    pub const fn new() -> Self {
        Self {
            pending: None,
            next: 0,
        }
    }
}
impl<const PORTS: usize> StepBack<PORTS> for StartupChimeBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if Some(request) != self.pending
                || outcome.output.is_some()
                || !match outcome.disposition {
                    HostCallDisposition::Completed => outcome.failure.is_none(),
                    HostCallDisposition::Denied => outcome
                        .failure
                        .is_some_and(|f| f.code == FailureCode::HostCallDenied),
                    HostCallDisposition::Failed => outcome
                        .failure
                        .is_some_and(|f| f.code == FailureCode::HostCallFailed),
                    HostCallDisposition::Cancelled => false,
                }
            {
                return invalid();
            }
            if io.consume_host_completion().is_err() {
                return invalid();
            }
            self.pending = None;
            let Some(next) = self.next.checked_add(1) else {
                return invalid();
            };
            self.next = next;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return invalid();
            }
            let Ok(input) = BoundedValueRef::new(value, conduit_core::BOOL_ENCODED_LEN as u32)
            else {
                return invalid();
            };
            if io.consume(PortId(0)).is_err()
                || io
                    .request_host_call(RequestId(self.next), HostCallId(0), input)
                    .is_err()
            {
                return invalid();
            }
            self.pending = Some(RequestId(self.next));
            return StepOutcome::Progress;
        }
        if self.pending.is_none() && io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return invalid();
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.pending = None;
    }
}
fn invalid() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{FixedValueStore, HostCallOutcome, ValueStorage};
    fn frame(
        input: Option<conduit_kernel::ValueRef>,
        closed: bool,
        completion: Option<(RequestId, HostCallOutcome)>,
    ) -> StepIo<1> {
        StepIo::test_frame([input], [closed], [None], completion, 32)
    }
    fn step(back: &mut StartupChimeBack, io: &mut StepIo<1>) -> StepOutcome {
        back.step(io, &StepInputBytes::test_frame([None], None))
    }
    fn begin() -> (StartupChimeBack, conduit_kernel::ValueRef) {
        let mut values = FixedValueStore::<1, 8>::new(8).unwrap();
        let value = values
            .store(&conduit_core::InfoBool::new(true).encode())
            .unwrap();
        let mut back = StartupChimeBack::new();
        let mut io = frame(Some(value), false, None);
        assert!(matches!(step(&mut back, &mut io), StepOutcome::Progress));
        assert!(io.test_consumed(PortId(0)));
        assert_eq!(io.test_host_request().unwrap().0, RequestId(0));
        (back, value)
    }
    #[test]
    fn exact_optional_completions_are_consumed_and_close_without_replay() {
        for (disposition, failure) in [
            (HostCallDisposition::Completed, None),
            (
                HostCallDisposition::Denied,
                Some(Failure {
                    code: FailureCode::HostCallDenied,
                    detail: 7,
                }),
            ),
            (
                HostCallDisposition::Failed,
                Some(Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 8,
                }),
            ),
        ] {
            let (mut back, _) = begin();
            let mut io = frame(
                None,
                true,
                Some((
                    RequestId(0),
                    HostCallOutcome {
                        disposition,
                        failure,
                        output: None,
                    },
                )),
            );
            assert!(matches!(step(&mut back, &mut io), StepOutcome::Progress));
            assert!(io.test_host_completion_consumed());
            assert!(io.test_host_request().is_none());
            assert_eq!(back.next, 1);
            let mut io = frame(None, true, None);
            assert!(matches!(step(&mut back, &mut io), StepOutcome::Complete));
        }
    }
    #[test]
    fn pending_closure_waits_and_cancellation_cannot_accept_a_late_completion() {
        let (mut back, value) = begin();
        let mut closed = frame(None, true, None);
        assert!(matches!(step(&mut back, &mut closed), StepOutcome::Await));
        assert!(closed.test_host_request().is_none());
        let mut repeated = frame(Some(value), false, None);
        assert!(matches!(
            step(&mut back, &mut repeated),
            StepOutcome::Fail(_)
        ));
        <StartupChimeBack as StepBack<1>>::cancel(&mut back);
        let mut idle = frame(None, false, None);
        assert!(matches!(step(&mut back, &mut idle), StepOutcome::Await));
        assert!(idle.test_host_request().is_none());
        let mut late = frame(
            None,
            false,
            Some((
                RequestId(0),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )),
        );
        assert!(matches!(step(&mut back, &mut late), StepOutcome::Fail(_)));
    }
    #[test]
    fn forged_completion_shapes_and_exhausted_generation_refuse() {
        for case in 0..5 {
            let (mut back, value) = begin();
            let mut outcome = HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            };
            let mut request = RequestId(0);
            match case {
                0 => request = RequestId(1),
                1 => outcome.output = Some(BoundedValueRef::new(value, 1).unwrap()),
                2 => outcome.disposition = HostCallDisposition::Cancelled,
                3 => outcome.disposition = HostCallDisposition::Denied,
                _ => back.next = u32::MAX,
            }
            let mut io = frame(None, false, Some((request, outcome)));
            assert!(
                matches!(step(&mut back, &mut io), StepOutcome::Fail(_)),
                "case {case}"
            );
        }
    }
}
