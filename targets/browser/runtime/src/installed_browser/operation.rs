//! Generic finite kernel verbs used by installed browser implementations.

use conduit_core::Scalar;
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::ValueStorage;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

pub(crate) struct BrowserOperation {
    step: Box<dyn StepBack<{ super::BROWSER_PORTS_PER_GEAR }>>,
}

impl BrowserOperation {
    pub(crate) fn installed_step(
        back: impl StepBack<{ super::BROWSER_PORTS_PER_GEAR }> + 'static,
    ) -> Self {
        Self {
            step: Box::new(back),
        }
    }

    pub(crate) fn source(value: ValueRef) -> Self {
        Self::installed_step(SourceOperation {
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
        Ok(Self::installed_step(HostSourceOperation {
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
        Ok(Self::installed_step(ApplicationStateOperation {
            initial,
            maximum_input_bytes,
            pending: None,
            next: 0,
        }))
    }

    pub(crate) fn unary(maximum_input_bytes: u32, _maximum_values: u32) -> Self {
        Self::installed_step(UnaryOperation {
            maximum_input_bytes,
            next_request: 0,
            pending: None,
        })
    }

    pub(crate) fn singleton_stream(maximum_bytes: u32) -> Self {
        Self::installed_step(SingletonStreamOperation {
            maximum_bytes,
            emitted: false,
        })
    }

    pub(crate) fn exactly_one(maximum_bytes: u32) -> Self {
        Self::installed_step(ExactlyOneOperation {
            maximum_bytes,
            held: None,
            emitted: false,
        })
    }

    pub(crate) fn presentation(maximum_input_bytes: u32, _maximum_values: u32) -> Self {
        Self::installed_step(PresentationOperation {
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
        Self::installed_step(CompareScalarOperation {
            operator,
            operands: [None, None],
            closed: [false; 2],
            decisions: [Some(false_value), Some(true_value)],
        })
    }

    pub(crate) fn inactive() -> Self {
        Self::installed_step(InactiveOperation)
    }

    pub(crate) fn select_scalar() -> Self {
        Self::installed_step(SelectScalarOperation {
            selector: None,
            selector_closed: false,
            candidates: [None; 2],
            seen: [false; 2],
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
}

impl ApplicationStateOperation {
    fn request<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        value: ValueRef,
        bound: u32,
    ) -> Result<(), StepOutcome> {
        let request = RequestId(self.next);
        match BoundedValueRef::new(value, bound) {
            Ok(input) => {
                io.request_host_call(request, HostCallId(0), input)
                    .expect("application state Host Call");
                self.pending = Some(request);
                Ok(())
            }
            Err(_) => Err(fail(5)),
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for ApplicationStateOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return fail(5);
            }
            let Some(output) = outcome.output else {
                return fail(5);
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(next) = self.next.checked_add(1) else {
                return identity_exhausted(5);
            };
            io.consume_host_completion()
                .expect("observed application state completion");
            io.send(PortId(0), output.value)
                .expect("ready application state output");
            self.pending = None;
            self.next = next;
            return StepOutcome::Progress;
        }
        if self.pending.is_none() && self.next == 0 {
            if let Err(outcome) = self.request(io, self.initial, 1) {
                return outcome;
            }
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return fail(5);
            }
            io.consume(PortId(0))
                .expect("present application state input");
            if let Err(outcome) = self.request(io, value, self.maximum_input_bytes) {
                return outcome;
            }
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed application state closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl HostSourceOperation {
    fn request<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        let request = RequestId(self.next);
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(self.request, 0).expect("source request is empty"),
        )
        .expect("browser source Host Call");
        self.pending = Some(request);
    }
}

impl<const PORTS: usize> StepBack<PORTS> for HostSourceOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return fail(4);
            }
            let Some(output) = outcome.output else {
                return fail(4);
            };
            if output.value.byte_len > self.maximum_output_bytes {
                return fail(4);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(next) = self.next.checked_add(1) else {
                return identity_exhausted(4);
            };
            io.consume_host_completion()
                .expect("observed browser source completion");
            io.send(PortId(0), output.value)
                .expect("ready browser source output");
            self.pending = None;
            self.next = next;
            self.request(io);
            return StepOutcome::Progress;
        }
        if self.pending.is_none() {
            self.request(io);
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl StepBack<{ super::BROWSER_PORTS_PER_GEAR }> for BrowserOperation {
    fn step_committed(&mut self) {
        self.step.step_committed();
    }

    fn step(
        &mut self,
        io: &mut StepIo<{ super::BROWSER_PORTS_PER_GEAR }>,
        input_bytes: &StepInputBytes<'_, { super::BROWSER_PORTS_PER_GEAR }>,
    ) -> StepOutcome {
        self.step.step(io, input_bytes)
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        self.step.accepts_input_while_host_call_pending()
    }

    fn retains_host_call_input(&self, request: RequestId, value: ValueRef) -> bool {
        self.step.retains_host_call_input(request, value)
    }

    fn cancel(&mut self) {
        self.step.cancel();
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

impl<const PORTS: usize> StepBack<PORTS> for SingletonStreamOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if self.emitted || value.byte_len > self.maximum_bytes {
                return fail(40);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).expect("present singleton input");
            io.send(PortId(0), value).expect("ready singleton output");
            self.emitted = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

struct ExactlyOneOperation {
    maximum_bytes: u32,
    held: Option<ValueRef>,
    emitted: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for ExactlyOneOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if self.held.is_some() || value.byte_len > self.maximum_bytes {
                return fail(41);
            }
            self.held = Some(io.take_input(PortId(0)).expect("present exactly-one input"));
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.emitted {
            let Some(value) = self.held else {
                return fail(41);
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_closed(PortId(0))
                .expect("observed exactly-one closure");
            io.send(PortId(0), value).expect("ready exactly-one output");
            self.held = None;
            self.emitted = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

impl<const PORTS: usize> StepBack<PORTS> for SourceOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return fail(1);
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), self.value)
            .expect("ready browser source output");
        self.emitted = true;
        StepOutcome::Complete
    }
}

struct UnaryOperation {
    maximum_input_bytes: u32,
    next_request: u32,
    pending: Option<RequestId>,
}

impl<const PORTS: usize> StepBack<PORTS> for UnaryOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return fail(2);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, output, None) => {
                    if output.is_some() && !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(next) = self.next_request.checked_add(1) else {
                        return identity_exhausted(2);
                    };
                    io.consume_host_completion()
                        .expect("observed unary Host Call completion");
                    if let Some(output) = output {
                        io.send(PortId(0), output.value)
                            .expect("ready unary output");
                    }
                    self.pending = None;
                    self.next_request = next;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Failed, None, Some(reason)) => {
                    self.pending = None;
                    return StepOutcome::Fail(reason);
                }
                (HostCallDisposition::Cancelled, None, None) => {
                    self.pending = None;
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    });
                }
                _ => return fail(2),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return fail(2);
            }
            let input = match BoundedValueRef::new(value, self.maximum_input_bytes) {
                Ok(input) => input,
                Err(_) => return fail(2),
            };
            let request = RequestId(self.next_request);
            io.consume(PortId(0)).expect("present unary input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("unary Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed unary input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
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

impl<const PORTS: usize> StepBack<PORTS> for PresentationOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return fail(3);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, None, None) => {
                    let Some(next) = self.next_request.checked_add(1) else {
                        return identity_exhausted(3);
                    };
                    io.consume_host_completion()
                        .expect("observed presentation completion");
                    self.pending = None;
                    self.next_request = next;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Failed | HostCallDisposition::Denied, None, Some(reason)) => {
                    return StepOutcome::Fail(reason)
                }
                _ => return fail(3),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return fail(3);
            }
            let input = match BoundedValueRef::new(value, self.maximum_input_bytes) {
                Ok(input) => input,
                Err(_) => return fail(3),
            };
            let request = RequestId(self.next_request);
            io.consume(PortId(0)).expect("present presentation input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("presentation Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed presentation input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
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
}

impl SelectScalarOperation {
    fn decide<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if !self.seen.into_iter().all(|seen| seen) {
            return StepOutcome::Await;
        }
        let Some(selector) = self.selector else {
            return if self.selector_closed {
                self.finish(io)
            } else {
                StepOutcome::Await
            };
        };
        let selected = usize::from(selector);
        let other = usize::from(!selector);
        let Some(value) = self.candidates[selected] else {
            return self.finish(io);
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), value)
            .expect("ready selected Scalar output");
        self.candidates[selected] = None;
        if let Some(unused) = self.candidates[other].take() {
            io.discard(unused).expect("unselected Scalar candidate");
        }
        StepOutcome::Complete
    }

    fn finish<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        for value in &mut self.candidates {
            if let Some(value) = value.take() {
                io.discard(value).expect("unused Scalar candidate");
            }
        }
        StepOutcome::Complete
    }
}

impl<const PORTS: usize> StepBack<PORTS> for SelectScalarOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.seen.into_iter().all(|seen| seen)
            && (self.selector.is_some() || self.selector_closed)
        {
            return self.decide(io);
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.selector.is_some()
                || self.selector_closed
                || value.byte_len != conduit_core::BOOL_ENCODED_LEN as u32
            {
                return fail(30);
            }
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return fail(30);
            };
            if canonical.len() != value.byte_len as usize {
                return fail(30);
            }
            let Ok(selector) = conduit_core::InfoBool::decode(canonical) else {
                return fail(30);
            };
            io.consume(PortId(0)).expect("present Scalar selector");
            self.selector = Some(selector.get());
            return StepOutcome::Progress;
        }
        for index in 0..2 {
            let port = PortId(index as u16 + 1);
            if let Some(value) = io.input(port) {
                if self.seen[index] || value.byte_len != conduit_core::SCALAR_ENCODED_LEN as u32 {
                    return fail(30);
                }
                let Some(canonical) = input_bytes.input(port) else {
                    return fail(30);
                };
                if canonical.len() != value.byte_len as usize
                    || conduit_core::Scalar::decode(canonical).is_err()
                {
                    return fail(30);
                }
                self.candidates[index] =
                    Some(io.take_input(port).expect("present Scalar candidate"));
                self.seen[index] = true;
                return StepOutcome::Progress;
            }
        }
        if io.input_closed(PortId(0)) && self.selector.is_none() && !self.selector_closed {
            io.consume_closed(PortId(0))
                .expect("observed Scalar selector closure");
            self.selector_closed = true;
            return StepOutcome::Progress;
        }
        for index in 0..2 {
            let port = PortId(index as u16 + 1);
            if io.input_closed(port) && !self.seen[index] {
                io.consume_closed(port)
                    .expect("observed Scalar candidate closure");
                self.seen[index] = true;
                return StepOutcome::Progress;
            }
        }
        StepOutcome::Await
    }
}

struct CompareScalarOperation {
    operator: conduit_semantic_catalog::ScalarComparison,
    operands: [Option<Scalar>; 2],
    closed: [bool; 2],
    decisions: [Option<ValueRef>; 2],
}

impl<const PORTS: usize> StepBack<PORTS> for CompareScalarOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        let [Some(left), Some(right)] = self.operands else {
            for index in 0..2 {
                let port = PortId(index as u16);
                if let Some(value) = io.input(port) {
                    if self.operands[index].is_some()
                        || self.closed[index]
                        || value.byte_len != conduit_core::SCALAR_ENCODED_LEN as u32
                    {
                        return fail(5);
                    }
                    let Some(canonical) = input_bytes.input(port) else {
                        return fail(5);
                    };
                    if canonical.len() != value.byte_len as usize {
                        return fail(5);
                    }
                    let Ok(value) = Scalar::decode(canonical) else {
                        return fail(5);
                    };
                    io.consume(port).expect("present Scalar operand");
                    self.operands[index] = Some(value);
                    return StepOutcome::Progress;
                }
            }
            for index in 0..2 {
                let port = PortId(index as u16);
                if io.input_closed(port) && !self.closed[index] {
                    if self.operands[index].is_none() {
                        io.consume_closed(port)
                            .expect("observed missing Scalar operand closure");
                        for decision in &mut self.decisions {
                            if let Some(value) = decision.take() {
                                io.discard(value).expect("unused comparison decision");
                            }
                        }
                        return StepOutcome::Complete;
                    }
                    io.consume_closed(port)
                        .expect("observed Scalar operand closure");
                    self.closed[index] = true;
                    return StepOutcome::Progress;
                }
            }
            return StepOutcome::Await;
        };
        let selected = usize::from(self.operator.evaluate(left, right));
        let unused = 1 - selected;
        let Some(value) = self.decisions[selected] else {
            return fail(5);
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), value)
            .expect("ready Scalar comparison output");
        self.decisions[selected] = None;
        if let Some(value) = self.decisions[unused].take() {
            io.discard(value).expect("unused comparison decision");
        }
        StepOutcome::Complete
    }

    fn cancel(&mut self) {
        self.decisions = [None, None];
    }
}

impl<const PORTS: usize> StepBack<PORTS> for InactiveOperation {
    fn step(&mut self, _: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        StepOutcome::Complete
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

fn identity_exhausted(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::IdentityCapacityExhausted,
        detail,
    })
}

#[cfg(test)]
#[path = "unary_outcome_tests.rs"]
mod unary_outcome_tests;
