//! Shared kernel lifecycle for one exact pure expression operation.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub struct PureExpressionBack {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for PureExpressionBack {
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
                        .expect("observed pure expression completion");
                    io.send(PortId(0), output.value)
                        .expect("ready pure expression output");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return failure(FailureCode::Cancelled, 0)
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
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
                .expect("present pure expression input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("pure expression Host Call");
            self.next_request = next_request;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed pure expression input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl PureExpressionBack {
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
