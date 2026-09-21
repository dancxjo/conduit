use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};
pub struct FrameOperation {
    pub input: Option<PortId>,
    pub output: Option<(PortId, ValueRef)>,
    pub operation: Option<HostCallId>,
    pub pending: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for FrameOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending
                || request != RequestId(1)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
                || outcome.output.is_some()
            {
                return failed();
            }
            if self.output.is_some_and(|(port, _)| !io.output_ready(port)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed frame Host Call completion");
            self.pending = false;
            return self.emit(io);
        }
        if let Some(port) = self.input {
            if let Some(value) = io.input(port) {
                if self.pending {
                    return failed();
                }
                let input = BoundedValueRef::new(value, 512).expect("bounded frame reference");
                io.consume(port).expect("present frame reference");
                io.request_host_call(
                    RequestId(1),
                    self.operation.expect("planned frame Host Call"),
                    input,
                )
                .expect("frame Host Call");
                self.pending = true;
                return StepOutcome::Progress;
            }
            return StepOutcome::Await;
        }
        if self.output.is_some_and(|(port, _)| !io.output_ready(port)) {
            return StepOutcome::Await;
        }
        self.emit(io)
    }
}
impl FrameOperation {
    fn emit<const PORTS: usize>(&self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some((port, value)) = self.output {
            io.send(port, value).expect("ready frame output");
        }
        StepOutcome::Complete
    }
}

fn failed() -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidInput,
        detail: 1,
    })
}
