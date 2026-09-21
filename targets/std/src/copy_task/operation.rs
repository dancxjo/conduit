use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

pub(crate) struct CopyBack {
    command: ValueRef,
    next_request: u32,
    pending: Option<RequestId>,
    emitted: bool,
}

impl CopyBack {
    pub(crate) fn new(command: ValueRef) -> Self {
        Self {
            command,
            next_request: 0,
            pending: None,
            emitted: false,
        }
    }

    fn request<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        let request = RequestId(self.next_request);
        let Some(next_request) = self.next_request.checked_add(1) else {
            return Self::fail(7);
        };
        let input = BoundedValueRef::new(self.command, conduit_std_offers::COPY_COMMAND_BYTES)
            .expect("copy command has one admitted byte");
        if io.request_host_call(request, HostCallId(0), input).is_err() {
            return Self::fail(6);
        }
        self.next_request = next_request;
        self.pending = Some(request);
        StepOutcome::Progress
    }

    fn fail(detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure {
            code: FailureCode::HostCallFailed,
            detail,
        })
    }
}

pub(crate) struct CopyResultBack {
    pending: bool,
    complete: bool,
}

impl CopyResultBack {
    pub(crate) const fn new() -> Self {
        Self {
            pending: false,
            complete: false,
        }
    }
}

pub(crate) enum CopyTaskBack {
    Copy(CopyBack),
    Sink(CopyResultBack),
}

impl<const PORTS: usize> StepOperation<PORTS> for CopyTaskBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Copy(copy) => {
                let Some(expected) = copy.pending else {
                    return if copy.emitted {
                        StepOutcome::Complete
                    } else {
                        copy.request(io)
                    };
                };
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != expected {
                    return CopyBack::fail(2);
                }
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None)
                        if output.value == copy.command =>
                    {
                        if io.consume_host_completion().is_err() {
                            return CopyBack::fail(6);
                        }
                        copy.pending = None;
                        copy.request(io)
                    }
                    (HostCallDisposition::Completed, Some(output), None) if !copy.emitted => {
                        if !io.output_ready(PortId(0)) {
                            return StepOutcome::Await;
                        }
                        if io.consume_host_completion().is_err()
                            || io.send(PortId(0), output.value).is_err()
                        {
                            return CopyBack::fail(6);
                        }
                        copy.pending = None;
                        copy.emitted = true;
                        StepOutcome::Progress
                    }
                    (HostCallDisposition::Completed, None, None) => {
                        if io.consume_host_completion().is_err() {
                            return CopyBack::fail(6);
                        }
                        copy.pending = None;
                        StepOutcome::Complete
                    }
                    (HostCallDisposition::Denied, None, _) => CopyBack::fail(3),
                    (HostCallDisposition::Cancelled, None, _) => CopyBack::fail(4),
                    (HostCallDisposition::Failed, None, _) => CopyBack::fail(5),
                    _ => CopyBack::fail(6),
                }
            }
            Self::Sink(sink) => {
                if sink.pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    if request != RequestId(0)
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                        || io.consume_host_completion().is_err()
                    {
                        return CopyBack::fail(8);
                    }
                    sink.pending = false;
                    sink.complete = true;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    let input = BoundedValueRef::new(
                        value,
                        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                    )
                    .expect("copy result is bounded");
                    if io.consume(PortId(0)).is_err()
                        || io
                            .request_host_call(RequestId(0), HostCallId(0), input)
                            .is_err()
                    {
                        return CopyBack::fail(8);
                    }
                    sink.pending = true;
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    if !sink.complete || io.consume_closed(PortId(0)).is_err() {
                        return CopyBack::fail(8);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Copy(copy) => copy.pending = None,
            Self::Sink(sink) => sink.pending = false,
        }
    }
}
