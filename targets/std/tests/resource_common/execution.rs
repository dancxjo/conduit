use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};
pub struct FrameOperation {
    pub input: Option<PortId>,
    pub output: Option<(PortId, ValueRef)>,
    pub operation: Option<HostOperationId>,
}
impl Operation for FrameOperation {
    fn start(&mut self) -> OperationAction {
        if self.input.is_none() {
            self.emit()
        } else {
            OperationAction::Await
        }
    }
    fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value } if Some(port) == self.input => {
                OperationAction::RequestHostOperation {
                    request: RequestId(1),
                    operation: self.operation.unwrap(),
                    input: BoundedValueRef::new(value, 512).unwrap(),
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(1),
                outcome,
            } if outcome.disposition == HostOperationDisposition::Completed => self.emit(),
            _ => OperationAction::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidInput,
                detail: 1,
            }),
        }
    }
}
impl FrameOperation {
    fn emit(&self) -> OperationAction {
        self.output
            .map_or(OperationAction::Complete, |(port, value)| {
                OperationAction::Emit { port, value }
            })
    }
}
