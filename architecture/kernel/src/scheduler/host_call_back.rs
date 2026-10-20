//! One exact bounded Host Call, using the production kernel lifecycle.

use crate::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub struct HostCallBack {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for HostCallBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return failure(FailureCode::InvalidLifecycle, 181);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed bounded Host Call completion");
                    io.send(PortId(0), output.value)
                        .expect("ready bounded Host Call output");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return failure(FailureCode::Cancelled, 0)
                }
                (
                    HostCallDisposition::Failed | HostCallDisposition::Denied,
                    None,
                    Some(failure),
                ) => {
                    self.pending = None;
                    return StepOutcome::Fail(failure);
                }
                _ => return failure(FailureCode::InvalidLifecycle, 182),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return failure(FailureCode::InvalidLifecycle, 183);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return failure(FailureCode::InvalidInput, 184);
            };
            let request = RequestId(self.next_request);
            let Some(next_request) = self.next_request.checked_add(1) else {
                return failure(FailureCode::StorageExhausted, 185);
            };
            io.consume(PortId(0))
                .expect("present bounded Host Call input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("bounded Host Call request");
            self.next_request = next_request;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed bounded Host Call input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl HostCallBack {
    pub const fn new(maximum_input_bytes: u32) -> Self {
        Self {
            maximum_input_bytes,
            pending: None,
            next_request: 0,
        }
    }
}

const fn failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

#[cfg(feature = "alloc")]
impl<const PORTS: usize> super::prepared_session::ReusableStepBack<PORTS> for HostCallBack {
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn crate::ValueStorage) -> bool {
        self.pending.is_none() && self.next_request < u32::MAX
    }
    fn begin_next(&mut self, _: ()) {
        // Input closure is scheduler-owned. Keep consumed request generations;
        // old completions must remain stale across the invocation boundary.
    }
}

#[cfg(all(test, feature = "alloc"))]
mod reuse_tests {
    use super::*;
    use crate::scheduler::prepared_session::ReusableStepBack;
    #[test]
    fn rearm_preserves_request_generation_and_refuses_pending_or_exhausted() {
        let values = crate::FixedValueStore::<1, 8>::new(8).unwrap();
        let mut back = HostCallBack::new(8);
        back.next_request = 17;
        assert!(<HostCallBack as ReusableStepBack<1>>::preflight_next(
            &back,
            &(),
            &values
        ));
        <HostCallBack as ReusableStepBack<1>>::begin_next(&mut back, ());
        assert_eq!(back.next_request, 17);
        back.pending = Some(RequestId(16));
        assert!(!<HostCallBack as ReusableStepBack<1>>::preflight_next(
            &back,
            &(),
            &values
        ));
        back.pending = None;
        back.next_request = u32::MAX;
        assert!(!<HostCallBack as ReusableStepBack<1>>::preflight_next(
            &back,
            &(),
            &values
        ));
    }
}
