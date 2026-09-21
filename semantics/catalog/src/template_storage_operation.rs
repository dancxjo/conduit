//! Shared finite kernel lifecycle for named-template commands.

use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub struct TemplateStorageOperation {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
    completed_commands: u64,
    maximum_commands: u64,
    closed: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for TemplateStorageOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return template_step_fail(FailureCode::InvalidLifecycle, 261);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed template-storage completion");
                    io.send(PortId(0), output.value)
                        .expect("ready template-storage output");
                    self.pending = None;
                    self.completed_commands += 1;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return template_step_fail(FailureCode::Cancelled, 0)
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return template_step_fail(FailureCode::InvalidLifecycle, 260),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.completed_commands >= self.maximum_commands {
                return template_step_fail(FailureCode::StorageExhausted, 262);
            }
            if self.pending.is_some() || self.closed {
                return template_step_fail(FailureCode::InvalidLifecycle, 261);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return template_step_fail(FailureCode::InvalidInput, 264);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return template_step_fail(FailureCode::StorageExhausted, 263);
            };
            io.consume(PortId(0))
                .expect("present template-storage command");
            io.request_host_call(request, HostCallId(0), input)
                .expect("template-storage Host Call");
            self.next_request = next;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() && !self.closed {
            io.consume_closed(PortId(0))
                .expect("observed template-storage closure");
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn template_step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl TemplateStorageOperation {
    pub fn new(maximum_commands: u64, maximum_input_bytes: u32) -> Self {
        Self {
            maximum_input_bytes,
            pending: None,
            next_request: 0,
            completed_commands: 0,
            maximum_commands,
            closed: false,
        }
    }
}
