//! Shared kernel lifecycle for exact structured selectors, including flow drops.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub struct StructuredSelectorBack {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
    complete: bool,
    cancelled: bool,
    staged_complete: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for StructuredSelectorBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged_complete = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 0,
            });
        }
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(143);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed structured selector completion");
                    io.send(PortId(0), output.value)
                        .expect("ready structured selector output");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed dropped structured selection");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    })
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return step_fail(142),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return step_fail(143);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return step_fail(141);
            };
            let request = RequestId(self.next_request);
            let Some(next_request) = self.next_request.checked_add(1) else {
                return step_fail(140);
            };
            io.consume(PortId(0))
                .expect("present structured selector input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("structured selector Host Call");
            self.next_request = next_request;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed structured selector closure");
            self.staged_complete = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.cancelled = true;
        self.staged_complete = false;
    }
    fn step_committed(&mut self) {
        self.complete |= self.staged_complete;
        self.staged_complete = false;
    }
}

const fn step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl StructuredSelectorBack {
    pub fn new(maximum_input_bytes: u32) -> Self {
        Self {
            maximum_input_bytes,
            pending: None,
            next_request: 0,
            complete: false,
            cancelled: false,
            staged_complete: false,
        }
    }
}
#[cfg(feature = "prepared-expression-session")]
impl<const P: usize> conduit_kernel::scheduler::prepared_session::ReusableStepBack<P>
    for StructuredSelectorBack
{
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn conduit_kernel::ValueStorage) -> bool {
        self.complete
            && !self.cancelled
            && !self.staged_complete
            && self.pending.is_none()
            && self.next_request < u32::MAX
    }
    fn begin_next(&mut self, _: ()) {
        self.complete = false;
    }
}
