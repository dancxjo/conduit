use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, RequestId, ValueRef,
};

pub(crate) struct CopyOperation {
    command: ValueRef,
    next_request: u32,
    pending: Option<RequestId>,
}

impl CopyOperation {
    pub(crate) fn new(command: ValueRef) -> Self {
        Self {
            command,
            next_request: 0,
            pending: None,
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
}
