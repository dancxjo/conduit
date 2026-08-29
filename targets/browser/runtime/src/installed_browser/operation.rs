//! Generic finite kernel verbs used by installed browser implementations.

use conduit_core::Scalar;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

pub(crate) struct BrowserOperation(Box<dyn Operation>);

impl BrowserOperation {
    pub(crate) fn source(value: ValueRef) -> Self {
        Self(Box::new(SourceOperation {
            value,
            emitted: false,
        }))
    }

    pub(crate) fn unary(maximum_input_bytes: u32, maximum_values: u32) -> Self {
        Self(Box::new(UnaryOperation {
            maximum_input_bytes,
            maximum_values,
            next: 0,
            pending: None,
        }))
    }

    pub(crate) fn presentation(maximum_input_bytes: u32, maximum_values: u32) -> Self {
        Self(Box::new(PresentationOperation {
            maximum_input_bytes,
            maximum_values,
            next: 0,
            pending: None,
        }))
    }

    pub(crate) fn compare_scalar(
        operator: conduit_semantic_catalog::ScalarComparison,
        false_value: ValueRef,
        true_value: ValueRef,
    ) -> Self {
        Self(Box::new(CompareScalarOperation {
            operator,
            operands: [None, None],
            decisions: [Some(false_value), Some(true_value)],
            released: [None, None],
            decided: false,
        }))
    }

    pub(crate) fn inactive() -> Self {
        Self(Box::new(InactiveOperation))
    }
}

impl Operation for BrowserOperation {
    fn start(&mut self) -> OperationAction {
        self.0.start()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.0.resume(input)
    }

    fn accepts_input_while_host_operation_pending(&self) -> bool {
        self.0.accepts_input_while_host_operation_pending()
    }

    fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        self.0.take_host_operation_cancellation()
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        self.0.resume_value(port, value, canonical)
    }

    fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: conduit_kernel::HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        self.0.resume_host_operation(request, outcome, canonical)
    }

    fn advance(&mut self) -> OperationAction {
        self.0.advance()
    }

    fn retains_resumed_value(&self) -> bool {
        self.0.retains_resumed_value()
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        self.0.take_released_value()
    }

    fn cancel(&mut self) {
        self.0.cancel();
    }
}

struct SourceOperation {
    value: ValueRef,
    emitted: bool,
}

impl Operation for SourceOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Emit {
            port: PortId(0),
            value: self.value,
        }
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        fail(1)
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            fail(1)
        } else {
            self.emitted = true;
            OperationAction::Complete
        }
    }
}

struct UnaryOperation {
    maximum_input_bytes: u32,
    maximum_values: u32,
    next: u32,
    pending: Option<RequestId>,
}

impl Operation for UnaryOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && self.next < self.maximum_values => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(2);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return fail(2);
                };
                self.pending = None;
                self.next = self.next.saturating_add(1);
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => fail(2),
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

struct PresentationOperation {
    maximum_input_bytes: u32,
    maximum_values: u32,
    next: u32,
    pending: Option<RequestId>,
}

impl Operation for PresentationOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && self.next < self.maximum_values => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(3);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.next = self.next.saturating_add(1);
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => fail(3),
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

struct InactiveOperation;

struct CompareScalarOperation {
    operator: conduit_semantic_catalog::ScalarComparison,
    operands: [Option<Scalar>; 2],
    decisions: [Option<ValueRef>; 2],
    released: [Option<ValueRef>; 2],
    decided: bool,
}

impl Operation for CompareScalarOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        let index = usize::from(port.0);
        if index >= self.operands.len()
            || self.operands[index].is_some()
            || value.byte_len != conduit_core::SCALAR_ENCODED_LEN as u32
        {
            return fail(5);
        }
        let Ok(value) = Scalar::decode(canonical) else {
            return fail(5);
        };
        self.operands[index] = Some(value);
        let [Some(left), Some(right)] = self.operands else {
            return OperationAction::Await;
        };
        let selected = usize::from(self.operator.evaluate(left, right));
        let unused = usize::from(!self.operator.evaluate(left, right));
        let Some(value) = self.decisions[selected].take() else {
            return fail(5);
        };
        self.released[0] = self.decisions[unused].take();
        self.decided = true;
        OperationAction::Emit {
            port: PortId(0),
            value,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port } if usize::from(port.0) < self.operands.len() => {
                if self.operands[usize::from(port.0)].is_none() {
                    self.released = [self.decisions[0].take(), self.decisions[1].take()];
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => fail(5),
        }
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.iter_mut().find_map(Option::take)
    }

    fn advance(&mut self) -> OperationAction {
        if self.decided {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.decisions = [None, None];
        self.released = [None, None];
        self.decided = true;
    }
}

impl Operation for InactiveOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Complete
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        fail(4)
    }
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
