use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, RequestId, ValueRef,
};

pub(crate) struct CopyOperation {
    command: ValueRef,
    next_request: u32,
    pending: Option<RequestId>,
    emitted: bool,
}

impl CopyOperation {
    pub(crate) fn new(command: ValueRef) -> Self {
        Self {
            command,
            next_request: 0,
            pending: None,
            emitted: false,
        }
    }

    fn request(&mut self) -> OperationAction {
        let request = RequestId(self.next_request);
        let Some(next_request) = self.next_request.checked_add(1) else {
            return Self::fail(7);
        };
        self.next_request = next_request;
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.command, conduit_std_catalog::COPY_COMMAND_BYTES)
                .expect("copy command has one admitted byte"),
        }
    }

    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::HostOperationFailed,
            detail,
        })
    }
}

impl Operation for CopyOperation {
    fn start(&mut self) -> OperationAction {
        self.request()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        let OperationInput::HostOperationCompleted { request, outcome } = input else {
            return Self::fail(1);
        };
        if self.pending != Some(request) {
            return Self::fail(2);
        }
        self.pending = None;
        match (outcome.disposition, outcome.output, outcome.failure) {
            (HostOperationDisposition::Completed, Some(output), None)
                if output.value == self.command =>
            {
                self.request()
            }
            (HostOperationDisposition::Completed, Some(output), None) if !self.emitted => {
                self.emitted = true;
                OperationAction::Emit {
                    port: conduit_kernel::PortId(0),
                    value: output.value,
                }
            }
            (HostOperationDisposition::Completed, None, None) => OperationAction::Complete,
            (HostOperationDisposition::Denied, None, _) => Self::fail(3),
            (HostOperationDisposition::Cancelled, None, _) => Self::fail(4),
            (HostOperationDisposition::Failed, None, _) => Self::fail(5),
            _ => Self::fail(6),
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
}

pub(crate) struct CopyResultSink {
    pending: bool,
    complete: bool,
}

impl CopyResultSink {
    pub(crate) const fn new() -> Self {
        Self {
            pending: false,
            complete: false,
        }
    }
}

impl Operation for CopyResultSink {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: conduit_kernel::PortId(0),
                value,
            } if !self.pending => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(
                        value,
                        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                    )
                    .expect("copy result is bounded"),
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                self.pending = false;
                self.complete = true;
                OperationAction::Await
            }
            OperationInput::Closed {
                port: conduit_kernel::PortId(0),
            } if self.complete && !self.pending => OperationAction::Complete,
            _ => CopyOperation::fail(8),
        }
    }

    fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

pub(crate) enum CopyTaskOperation {
    Copy(CopyOperation),
    Sink(CopyResultSink),
}

impl Operation for CopyTaskOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Copy(value) => value.start(),
            Self::Sink(value) => value.start(),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Copy(value) => value.resume(input),
            Self::Sink(value) => value.resume(input),
        }
    }
    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Copy(value) => value.advance(),
            Self::Sink(value) => value.advance(),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Copy(value) => value.cancel(),
            Self::Sink(value) => value.cancel(),
        }
    }
}
