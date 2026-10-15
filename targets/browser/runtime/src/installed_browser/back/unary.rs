//! Bounded unary Host Call progression with an optional admitted initial Current.
use super::{fail, identity_exhausted};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

pub(super) struct UnaryBack {
    pub(super) initial: Option<ValueRef>,
    pub(super) maximum_input_bytes: u32,
    pub(super) next_request: u32,
    pub(super) pending: Option<RequestId>,
}

impl<const PORTS: usize> StepBack<PORTS> for UnaryBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(initial) = self.initial {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send(PortId(0), initial)
                .expect("ready initial Current output");
            self.initial = None;
            return StepOutcome::Progress;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return fail(2);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, output, None) => {
                    if output.is_some() && !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(next) = self.next_request.checked_add(1) else {
                        return identity_exhausted(2);
                    };
                    io.consume_host_completion()
                        .expect("observed unary Host Call completion");
                    if let Some(output) = output {
                        io.send(PortId(0), output.value)
                            .expect("ready unary output");
                    }
                    self.pending = None;
                    self.next_request = next;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Failed, None, Some(reason)) => {
                    self.pending = None;
                    return StepOutcome::Fail(reason);
                }
                (HostCallDisposition::Cancelled, None, None) => {
                    self.pending = None;
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    });
                }
                _ => return fail(2),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return fail(2);
            }
            let input = match BoundedValueRef::new(value, self.maximum_input_bytes) {
                Ok(input) => input,
                Err(_) => return fail(2),
            };
            let request = RequestId(self.next_request);
            io.consume(PortId(0)).expect("present unary input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("unary Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed unary input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.initial = None;
        self.pending = None;
    }
}

#[cfg(test)]
#[path = "unary_tests.rs"]
mod tests;
