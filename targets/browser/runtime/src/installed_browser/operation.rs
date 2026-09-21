//! Generic finite kernel verbs used by installed browser implementations.

use conduit_core::Scalar;
use conduit_kernel::scheduler::{
    OperationDriver, StepInputBytes, StepIo, StepOperation, StepOutcome,
};
use conduit_kernel::ValueStorage;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

struct LegacyBrowserOperation(Box<dyn Operation>);

pub(crate) struct BrowserOperation {
    legacy: Option<LegacyBrowserOperation>,
    step: Option<OperationDriver<LegacyBrowserOperation, { super::BROWSER_PORTS_PER_GEAR }>>,
    native: Option<Box<dyn StepOperation<{ super::BROWSER_PORTS_PER_GEAR }>>>,
}

impl BrowserOperation {
    pub(crate) fn installed(operation: impl Operation + 'static) -> Self {
        Self {
            legacy: Some(LegacyBrowserOperation(Box::new(operation))),
            step: None,
            native: None,
        }
    }

    pub(crate) fn installed_step(
        back: impl StepOperation<{ super::BROWSER_PORTS_PER_GEAR }> + 'static,
    ) -> Self {
        Self {
            legacy: None,
            step: None,
            native: Some(Box::new(back)),
        }
    }

    pub(crate) fn source(value: ValueRef) -> Self {
        Self::installed(SourceOperation {
            value,
            emitted: false,
        })
    }

    pub(crate) fn host_source(
        values: &mut conduit_kernel::HostedValueStore,
        maximum_output_bytes: u32,
    ) -> Result<Self, String> {
        let request = values
            .store(&[])
            .map_err(|error| format!("store browser source request: {error:?}"))?;
        Ok(Self::installed(HostSourceOperation {
            request,
            maximum_output_bytes,
            pending: None,
            next: 0,
        }))
    }

    pub(crate) fn application_state(
        values: &mut conduit_kernel::HostedValueStore,
        maximum_input_bytes: u32,
    ) -> Result<Self, String> {
        let initial = values
            .store(&[0])
            .map_err(|error| format!("store application initial request: {error:?}"))?;
        Ok(Self::installed(ApplicationStateOperation {
            initial,
            maximum_input_bytes,
            pending: None,
            next: 0,
            initial_sent: false,
        }))
    }

    pub(crate) fn unary(maximum_input_bytes: u32, _maximum_values: u32) -> Self {
        Self::installed(UnaryOperation {
            maximum_input_bytes,
            next_request: 0,
            pending: None,
        })
    }

    pub(crate) fn singleton_stream(maximum_bytes: u32) -> Self {
        Self::installed(SingletonStreamOperation {
            maximum_bytes,
            emitted: false,
        })
    }

    pub(crate) fn exactly_one(maximum_bytes: u32) -> Self {
        Self::installed(ExactlyOneOperation {
            maximum_bytes,
            held: None,
            released: None,
            emitted: false,
            retain_resumed: false,
        })
    }

    pub(crate) fn presentation(maximum_input_bytes: u32, _maximum_values: u32) -> Self {
        Self::installed(PresentationOperation {
            maximum_input_bytes,
            next_request: 0,
            pending: None,
        })
    }

    pub(crate) fn compare_scalar(
        operator: conduit_semantic_catalog::ScalarComparison,
        false_value: ValueRef,
        true_value: ValueRef,
    ) -> Self {
        Self::installed(CompareScalarOperation {
            operator,
            operands: [None, None],
            decisions: [Some(false_value), Some(true_value)],
            released: [None, None],
            decided: false,
        })
    }

    pub(crate) fn inactive() -> Self {
        Self::installed(InactiveOperation)
    }

    pub(crate) fn select_scalar() -> Self {
        Self::installed(SelectScalarOperation {
            selector: None,
            selector_closed: false,
            candidates: [None; 2],
            seen: [false; 2],
            released: [None; 2],
            retain_resumed: false,
        })
    }
}

struct HostSourceOperation {
    request: ValueRef,
    maximum_output_bytes: u32,
    pending: Option<RequestId>,
    next: u32,
}

struct ApplicationStateOperation {
    initial: ValueRef,
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next: u32,
    initial_sent: bool,
}

impl ApplicationStateOperation {
    fn request(&mut self, value: ValueRef, bound: u32) -> OperationAction {
        let request = RequestId(self.next);
        self.pending = Some(request);
        match BoundedValueRef::new(value, bound) {
            Ok(input) => OperationAction::RequestHostCall {
                request,
                operation: HostCallId(0),
                input,
            },
            Err(_) => fail(5),
        }
    }
}

impl Operation for ApplicationStateOperation {
    fn start(&mut self) -> OperationAction {
        self.request(self.initial, 1)
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => self.request(value, self.maximum_input_bytes),
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                let Some(next) = self.next.checked_add(1) else {
                    return identity_exhausted(5);
                };
                self.next = next;
                self.initial_sent = true;
                match outcome.output {
                    Some(output) => OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    },
                    None => fail(5),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => fail(5),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.initial_sent {
            OperationAction::Await
        } else {
            fail(5)
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl HostSourceOperation {
    fn request(&mut self) -> OperationAction {
        let request = RequestId(self.next);
        self.pending = Some(request);
        OperationAction::RequestHostCall {
            request,
            operation: HostCallId(0),
            input: BoundedValueRef::new(self.request, 0).expect("source request is empty"),
        }
    }
}

impl Operation for HostSourceOperation {
    fn start(&mut self) -> OperationAction {
        self.request()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                let Some(output) = outcome.output else {
                    return fail(4);
                };
                if output.value.byte_len > self.maximum_output_bytes {
                    return fail(4);
                }
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            _ => fail(4),
        }
    }

    fn advance(&mut self) -> OperationAction {
        let Some(next) = self.next.checked_add(1) else {
            return identity_exhausted(4);
        };
        self.next = next;
        self.request()
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl Operation for LegacyBrowserOperation {
    fn start(&mut self) -> OperationAction {
        self.0.start()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.0.resume(input)
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        self.0.accepts_input_while_host_call_pending()
    }

    fn retains_host_call_input(&self, request: RequestId, value: ValueRef) -> bool {
        self.0.retains_host_call_input(request, value)
    }

    fn take_host_call_cancellation(&mut self) -> Option<RequestId> {
        self.0.take_host_call_cancellation()
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        self.0.resume_value(port, value, canonical)
    }

    fn resume_host_call(
        &mut self,
        request: RequestId,
        outcome: conduit_kernel::HostCallOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        self.0.resume_host_call(request, outcome, canonical)
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

impl StepOperation<{ super::BROWSER_PORTS_PER_GEAR }> for BrowserOperation {
    fn step_committed(&mut self) {
        if let Some(native) = self.native.as_mut() {
            native.step_committed();
            return;
        }
        if let Some(step) = self.step.as_mut() {
            step.step_committed();
        }
    }

    fn step(
        &mut self,
        io: &mut StepIo<{ super::BROWSER_PORTS_PER_GEAR }>,
        input_bytes: &StepInputBytes<'_, { super::BROWSER_PORTS_PER_GEAR }>,
    ) -> StepOutcome {
        if let Some(native) = self.native.as_mut() {
            return native.step(io, input_bytes);
        }
        if self.step.is_none() {
            let legacy = self.legacy.take().expect("browser Back initializes once");
            match OperationDriver::new(legacy) {
                Ok(step) => self.step = Some(step),
                Err(_) => {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::InvalidLifecycle,
                        detail: u16::MAX,
                    });
                }
            }
        }
        self.step
            .as_mut()
            .expect("browser Step initialized")
            .step(io, input_bytes)
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        if let Some(native) = self.native.as_ref() {
            return native.accepts_input_while_host_call_pending();
        }
        self.step
            .as_ref()
            .is_some_and(StepOperation::accepts_input_while_host_call_pending)
    }

    fn retains_host_call_input(&self, request: RequestId, value: ValueRef) -> bool {
        if let Some(native) = self.native.as_ref() {
            return native.retains_host_call_input(request, value);
        }
        self.step
            .as_ref()
            .is_some_and(|step| StepOperation::retains_host_call_input(step, request, value))
    }

    fn cancel(&mut self) {
        if let Some(native) = self.native.as_mut() {
            native.cancel();
        } else if let Some(step) = self.step.as_mut() {
            step.cancel();
        } else if let Some(legacy) = self.legacy.as_mut() {
            legacy.cancel();
        }
    }
}

// The remaining unit tests exercise the old verb-level fixtures directly.
// Production browser execution enters only through `StepOperation` above.
#[cfg(test)]
impl Operation for BrowserOperation {
    fn start(&mut self) -> OperationAction {
        self.legacy
            .as_mut()
            .expect("unstarted browser fixture")
            .start()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.legacy
            .as_mut()
            .expect("unstarted browser fixture")
            .resume(input)
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        self.legacy
            .as_mut()
            .expect("unstarted browser fixture")
            .resume_value(port, value, canonical)
    }

    fn advance(&mut self) -> OperationAction {
        self.legacy
            .as_mut()
            .expect("unstarted browser fixture")
            .advance()
    }
}

struct SourceOperation {
    value: ValueRef,
    emitted: bool,
}

struct SingletonStreamOperation {
    maximum_bytes: u32,
    emitted: bool,
}

impl Operation for SingletonStreamOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.emitted && value.byte_len <= self.maximum_bytes => {
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            _ => fail(40),
        }
    }

    fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }
}

struct ExactlyOneOperation {
    maximum_bytes: u32,
    held: Option<ValueRef>,
    released: Option<ValueRef>,
    emitted: bool,
    retain_resumed: bool,
}

impl Operation for ExactlyOneOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.held.is_none() && value.byte_len <= self.maximum_bytes => {
                self.held = Some(value);
                self.retain_resumed = true;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if !self.emitted => {
                let Some(value) = self.held.take() else {
                    return fail(41);
                };
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            _ => fail(41),
        }
    }

    fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.take()
    }

    fn cancel(&mut self) {
        self.released = self.held.take();
        self.retain_resumed = false;
    }
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
    next_request: u32,
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
            } if self.pending.is_none() => {
                let request = RequestId(self.next_request);
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(2);
                };
                OperationAction::RequestHostCall {
                    request,
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                let Some(next) = self.next_request.checked_add(1) else {
                    return identity_exhausted(2);
                };
                self.next_request = next;
                match outcome.output {
                    Some(output) => OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    },
                    None => OperationAction::Await,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Failed
                    && outcome.output.is_none() =>
            {
                self.pending = None;
                outcome
                    .failure
                    .map_or_else(|| fail(2), OperationAction::Fail)
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Cancelled
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                OperationAction::Fail(Failure {
                    code: FailureCode::Cancelled,
                    detail: 0,
                })
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
    next_request: u32,
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
            } if self.pending.is_none() => {
                let request = RequestId(self.next_request);
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(3);
                };
                OperationAction::RequestHostCall {
                    request,
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                let Some(next) = self.next_request.checked_add(1) else {
                    return identity_exhausted(3);
                };
                self.next_request = next;
                OperationAction::Await
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && matches!(
                        outcome.disposition,
                        HostCallDisposition::Failed | HostCallDisposition::Denied
                    )
                    && outcome.output.is_none() =>
            {
                self.pending = None;
                outcome
                    .failure
                    .map_or_else(|| fail(3), OperationAction::Fail)
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

struct SelectScalarOperation {
    selector: Option<bool>,
    selector_closed: bool,
    candidates: [Option<ValueRef>; 2],
    seen: [bool; 2],
    released: [Option<ValueRef>; 2],
    retain_resumed: bool,
}

impl SelectScalarOperation {
    fn decide(&mut self) -> OperationAction {
        if !self.seen.into_iter().all(|seen| seen) {
            return OperationAction::Await;
        }
        let Some(selector) = self.selector else {
            return if self.selector_closed {
                self.finish()
            } else {
                OperationAction::Await
            };
        };
        let selected = usize::from(selector);
        let other = usize::from(!selector);
        let Some(value) = self.candidates[selected].take() else {
            return self.finish();
        };
        self.released[0] = self.candidates[other].take();
        self.retain_resumed = false;
        OperationAction::Emit {
            port: PortId(0),
            value,
        }
    }

    fn finish(&mut self) -> OperationAction {
        self.released = [self.candidates[0].take(), self.candidates[1].take()];
        OperationAction::Complete
    }
}

impl Operation for SelectScalarOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Closed { port: PortId(0) } if self.selector.is_none() => {
                self.selector_closed = true;
                self.decide()
            }
            OperationInput::Closed {
                port: PortId(1) | PortId(2),
            } => {
                let OperationInput::Closed { port } = input else {
                    unreachable!()
                };
                let index = usize::from(port.0 - 1);
                if self.seen[index] {
                    return fail(30);
                }
                self.seen[index] = true;
                self.decide()
            }
            _ => fail(30),
        }
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        self.retain_resumed = false;
        match port {
            PortId(0)
                if self.selector.is_none()
                    && value.byte_len == conduit_core::BOOL_ENCODED_LEN as u32 =>
            {
                let Ok(selector) = conduit_core::InfoBool::decode(canonical) else {
                    return fail(30);
                };
                self.selector = Some(selector.get());
            }
            PortId(1) | PortId(2) if value.byte_len == conduit_core::SCALAR_ENCODED_LEN as u32 => {
                let index = usize::from(port.0 - 1);
                if self.seen[index] || conduit_core::Scalar::decode(canonical).is_err() {
                    return fail(30);
                }
                self.seen[index] = true;
                self.candidates[index] = Some(value);
                self.retain_resumed = true;
            }
            _ => return fail(30),
        }
        self.decide()
    }

    fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }
    fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }
    fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.iter_mut().find_map(Option::take)
    }
    fn cancel(&mut self) {
        let _ = self.finish();
    }
}

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

fn identity_exhausted(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::IdentityCapacityExhausted,
        detail,
    })
}

#[cfg(test)]
#[path = "unary_outcome_tests.rs"]
mod unary_outcome_tests;
