use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, Operation, OperationAction,
    OperationInput, PortId, RequestId,
};

pub(crate) struct JoinOperation {
    input_port: PortId,
    output_port: PortId,
    operation: conduit_kernel::HostOperationId,
    pending: Option<RequestId>,
    completed: bool,
}

impl Operation for JoinOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if port == self.input_port && self.pending.is_none() =>
            {
                let request = RequestId(0);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: self.operation,
                    input: BoundedValueRef::new(value, conduit_net::MAXIMUM_JOIN_INPUT_BYTES)
                        .expect("planned join input is exactly bounded"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_some()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.completed = true;
                OperationAction::Emit {
                    port: self.output_port,
                    value: outcome.output.expect("checked attachment output").value,
                }
            }
            OperationInput::Closed { port }
                if port == self.input_port && self.pending.is_none() && self.completed =>
            {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 1,
            }),
        }
    }
}

pub(crate) struct AttachmentClueOperation {
    input_port: PortId,
    operation: conduit_kernel::HostOperationId,
    pending: Option<RequestId>,
    completed: bool,
}

impl Operation for AttachmentClueOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if port == self.input_port && self.pending.is_none() =>
            {
                let request = RequestId(0);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: self.operation,
                    input: BoundedValueRef::new(value, conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES)
                        .expect("planned attachment Info is exactly bounded"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.completed = true;
                OperationAction::Await
            }
            OperationInput::Closed { port }
                if port == self.input_port && self.pending.is_none() && self.completed =>
            {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 2,
            }),
        }
    }
}

pub enum NetworkOperation {
    Join(JoinOperation),
    AttachmentClue(AttachmentClueOperation),
}

impl NetworkOperation {
    pub fn join(
        input_port: PortId,
        output_port: PortId,
        operation: conduit_kernel::HostOperationId,
    ) -> Self {
        Self::Join(JoinOperation {
            input_port,
            output_port,
            operation,
            pending: None,
            completed: false,
        })
    }

    pub fn attachment_clue(
        input_port: PortId,
        operation: conduit_kernel::HostOperationId,
    ) -> Self {
        Self::AttachmentClue(AttachmentClueOperation {
            input_port,
            operation,
            pending: None,
            completed: false,
        })
    }
}

impl Operation for NetworkOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Join(operation) => operation.start(),
            Self::AttachmentClue(operation) => operation.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Join(operation) => operation.resume(input),
            Self::AttachmentClue(operation) => operation.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Join(operation) => operation.advance(),
            Self::AttachmentClue(operation) => operation.advance(),
        }
    }
}
