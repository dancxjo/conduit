//! Shared finite two-input normalized-pattern comparison operation.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub struct PatternComparisonOperation {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
    received: [bool; 2],
    closed: [bool; 2],
}

impl<const PORTS: usize> StepBack<PORTS> for PatternComparisonOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_failure(FailureCode::InvalidLifecycle, 251);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None)
                    if self.received == [true, true] =>
                {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed pattern comparison completion");
                    io.send(PortId(0), output.value)
                        .expect("ready pattern comparison output");
                    self.pending = None;
                    self.received[0] = false;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed empty pattern comparison completion");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return step_failure(FailureCode::Cancelled, 0)
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return step_failure(FailureCode::InvalidLifecycle, 250),
            }
        }

        for port in [PortId(0), PortId(1)] {
            let Some(value) = io.input(port) else {
                continue;
            };
            let index = usize::from(port.0);
            if self.pending.is_some() || self.received[index] || self.closed[index] {
                return step_failure(FailureCode::InvalidLifecycle, 251);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return step_failure(FailureCode::InvalidInput, 254);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_failure(FailureCode::StorageExhausted, 253);
            };
            io.consume(port).expect("present pattern comparison input");
            io.request_host_call(request, HostCallId(port.0), input)
                .expect("pattern comparison Host Call");
            self.next_request = next;
            self.pending = Some(request);
            self.received[index] = true;
            return StepOutcome::Progress;
        }

        for port in [PortId(0), PortId(1)] {
            let index = usize::from(port.0);
            if io.input_closed(port) && !self.closed[index] {
                io.consume_closed(port)
                    .expect("observed pattern comparison closure");
                self.closed[index] = true;
                return if self.closed == [true, true] {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl PatternComparisonOperation {
    /// Construct before Play with the exact host-admitted input byte bound.
    pub fn new(maximum_input_bytes: u32) -> Self {
        Self {
            maximum_input_bytes,
            pending: None,
            next_request: 0,
            received: [false; 2],
            closed: [false; 2],
        }
    }
}
