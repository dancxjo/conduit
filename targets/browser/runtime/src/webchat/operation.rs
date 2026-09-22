use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const CLOSE: HostCallId = HostCallId(0);
const OPEN: HostCallId = HostCallId(1);
const RECEIVE: HostCallId = HostCallId(2);
const SEND: HostCallId = HostCallId(3);

pub(crate) enum BrowserChatBack {
    State(State),
    Tee,
    Renderer(Request),
    Interaction(Interaction),
    Submit(Request),
    Adapter(Request),
    Socket(Socket),
}

pub(crate) struct State {
    initial: Option<ValueRef>,
    pending: Option<RequestId>,
    next: u32,
}

pub(crate) struct Request {
    pending: Option<RequestId>,
    next: u32,
    maximum: u32,
}

pub(crate) struct Interaction {
    token: ValueRef,
    presentation: Option<ValueRef>,
    manifestation: Option<ValueRef>,
    pending: Option<RequestId>,
    next: u32,
}

pub(crate) struct Socket {
    open: Option<ValueRef>,
    receive: Option<ValueRef>,
    close: Option<ValueRef>,
    live: ValueRef,
    next: u32,
    pending: Option<(RequestId, HostCallId)>,
    opened: bool,
}

impl BrowserChatBack {
    pub(crate) fn state(initial: ValueRef) -> Self {
        Self::State(State {
            initial: Some(initial),
            pending: None,
            next: 0,
        })
    }

    pub(crate) fn tee() -> Self {
        Self::Tee
    }

    pub(crate) fn renderer() -> Self {
        Self::Renderer(Request::new(64 * 1024))
    }

    pub(crate) fn interaction(token: ValueRef) -> Self {
        Self::Interaction(Interaction {
            token,
            presentation: None,
            manifestation: None,
            pending: None,
            next: 0,
        })
    }

    pub(crate) fn submit() -> Self {
        Self::Submit(Request::new(
            conduit_presentation::MAX_PRESENTATION_INTERACTION_BYTES as u32,
        ))
    }

    pub(crate) fn adapter() -> Self {
        Self::Adapter(Request::new(conduit_chat::MAXIMUM_CHAT_MESSAGE_BYTES))
    }

    pub(crate) fn socket(
        open: ValueRef,
        receive: ValueRef,
        close: ValueRef,
        live: ValueRef,
    ) -> Self {
        Self::Socket(Socket {
            open: Some(open),
            receive: Some(receive),
            close: Some(close),
            live,
            next: 0,
            pending: None,
            opened: false,
        })
    }

    fn fail(detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl StepBack<PORTS> for BrowserChatBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::State(state) => state.step(io),
            Self::Tee => step_tee(io),
            Self::Renderer(request) | Self::Submit(request) | Self::Adapter(request) => {
                request.step(io)
            }
            Self::Interaction(interaction) => interaction.step(io),
            Self::Socket(socket) => socket.step(io),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::State(state) => state.pending = None,
            Self::Renderer(request) | Self::Submit(request) | Self::Adapter(request) => {
                request.pending = None;
            }
            Self::Interaction(interaction) => interaction.pending = None,
            Self::Socket(socket) => socket.pending = None,
            Self::Tee => {}
        }
    }
}

impl State {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(value) = self.initial {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.send(PortId(0), value).is_err() {
                return BrowserChatBack::fail(40);
            }
            self.initial = None;
            return StepOutcome::Progress;
        }

        if let Some(expected) = self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != expected
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return BrowserChatBack::fail(43);
            }
            let Some(output) = outcome.output else {
                return BrowserChatBack::fail(42);
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err() || io.send(PortId(0), output.value).is_err() {
                return BrowserChatBack::fail(43);
            }
            self.pending = None;
            return StepOutcome::Progress;
        }

        for port in [PortId(0), PortId(1)] {
            if let Some(value) = io.input(port) {
                let request = RequestId(self.next);
                let maximum = if port.0 == 0 {
                    conduit_chat::MAXIMUM_CHAT_MESSAGE_BYTES
                } else {
                    1
                };
                let input = BoundedValueRef::new(value, maximum).expect("bounded state input");
                if io.consume(port).is_err()
                    || io
                        .request_host_call(request, HostCallId(1 - port.0), input)
                        .is_err()
                {
                    return BrowserChatBack::fail(43);
                }
                self.next = self.next.saturating_add(1);
                self.pending = Some(request);
                return StepOutcome::Progress;
            }
        }
        StepOutcome::Await
    }
}

fn step_tee(io: &mut StepIo<PORTS>) -> StepOutcome {
    if let Some(value) = io.input(PortId(0)) {
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err() || io.send(PortId(0), value).is_err() {
            return BrowserChatBack::fail(41);
        }
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return BrowserChatBack::fail(41);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

impl Request {
    fn new(maximum: u32) -> Self {
        Self {
            pending: None,
            next: 0,
            maximum,
        }
    }

    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(expected) = self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != expected
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return BrowserChatBack::fail(44);
            }
            if let Some(output) = outcome.output {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.consume_host_completion().is_err()
                    || io.send(PortId(0), output.value).is_err()
                {
                    return BrowserChatBack::fail(44);
                }
            } else if io.consume_host_completion().is_err() {
                return BrowserChatBack::fail(44);
            }
            self.pending = None;
            return StepOutcome::Progress;
        }

        if let Some(value) = io.input(PortId(0)) {
            let request = RequestId(self.next);
            let input = BoundedValueRef::new(value, self.maximum).expect("bounded Back input");
            if io.consume(PortId(0)).is_err()
                || io.request_host_call(request, HostCallId(0), input).is_err()
            {
                return BrowserChatBack::fail(44);
            }
            self.next = self.next.saturating_add(1);
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return BrowserChatBack::fail(44);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

impl Interaction {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(expected) = self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != expected {
                return BrowserChatBack::fail(46);
            }
            match outcome.disposition {
                HostCallDisposition::Completed if outcome.failure.is_none() => {
                    if let Some(output) = outcome.output {
                        if !io.output_ready(PortId(0)) {
                            return StepOutcome::Await;
                        }
                        if io.consume_host_completion().is_err()
                            || io.send(PortId(0), output.value).is_err()
                        {
                            return BrowserChatBack::fail(46);
                        }
                    } else if io.consume_host_completion().is_err() {
                        return BrowserChatBack::fail(46);
                    }
                }
                HostCallDisposition::Cancelled => {
                    if io.consume_host_completion().is_err() {
                        return BrowserChatBack::fail(46);
                    }
                }
                _ => return BrowserChatBack::fail(45),
            }
            self.pending = None;
            return StepOutcome::Progress;
        }

        let mut consumed = false;
        if self.presentation.is_none() {
            if let Some(value) = io.input(PortId(0)) {
                if io.consume(PortId(0)).is_err() {
                    return BrowserChatBack::fail(46);
                }
                self.presentation = Some(value);
                consumed = true;
            }
        }
        if self.manifestation.is_none() {
            if let Some(value) = io.input(PortId(1)) {
                if io.consume(PortId(1)).is_err() {
                    return BrowserChatBack::fail(46);
                }
                self.manifestation = Some(value);
                consumed = true;
            }
        }
        if self.presentation.is_none() || self.manifestation.is_none() {
            return if consumed {
                StepOutcome::Progress
            } else {
                StepOutcome::Await
            };
        }

        let request = RequestId(self.next);
        let input = BoundedValueRef::new(self.token, 0).expect("empty admitted input token");
        if io.request_host_call(request, HostCallId(0), input).is_err() {
            return BrowserChatBack::fail(46);
        }
        self.next = self.next.saturating_add(1);
        self.pending = Some(request);
        StepOutcome::Progress
    }
}

impl Socket {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some((expected, host_call)) = self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != expected {
                return BrowserChatBack::fail(50);
            }
            return self.complete(host_call, outcome, io);
        }

        if self.opened {
            self.opened = false;
            let value = self.receive.take().unwrap_or(self.live);
            return self.request(
                RECEIVE,
                value,
                conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
                io,
            );
        }

        if let Some(value) = self.open.take() {
            return self.request(OPEN, value, 256, io);
        }

        if let Some(value) = io.input(PortId(0)) {
            let input =
                BoundedValueRef::new(value, conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES)
                    .expect("bounded socket input");
            let request = RequestId(self.next);
            if io.consume(PortId(0)).is_err() || io.request_host_call(request, SEND, input).is_err()
            {
                return BrowserChatBack::fail(54);
            }
            self.next = self.next.saturating_add(1);
            self.pending = Some((request, SEND));
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) {
            let Some(value) = self.close.take() else {
                return BrowserChatBack::fail(48);
            };
            let input = BoundedValueRef::new(value, 1).expect("bounded socket close input");
            let request = RequestId(self.next);
            if io.consume_closed(PortId(0)).is_err()
                || io.request_host_call(request, CLOSE, input).is_err()
            {
                return BrowserChatBack::fail(54);
            }
            self.next = self.next.saturating_add(1);
            self.pending = Some((request, CLOSE));
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn request(
        &mut self,
        host_call: HostCallId,
        value: ValueRef,
        maximum: u32,
        io: &mut StepIo<PORTS>,
    ) -> StepOutcome {
        let request = RequestId(self.next);
        let input = BoundedValueRef::new(value, maximum).expect("bounded socket input");
        if io.request_host_call(request, host_call, input).is_err() {
            return BrowserChatBack::fail(54);
        }
        self.next = self.next.saturating_add(1);
        self.pending = Some((request, host_call));
        StepOutcome::Progress
    }

    fn complete(
        &mut self,
        host_call: HostCallId,
        outcome: conduit_kernel::HostCallOutcome,
        io: &mut StepIo<PORTS>,
    ) -> StepOutcome {
        match host_call {
            OPEN if outcome.disposition == HostCallDisposition::Completed
                && outcome.failure.is_none() =>
            {
                if !io.output_ready(PortId(1)) || io.consume_host_completion().is_err() {
                    return if io.output_ready(PortId(1)) {
                        BrowserChatBack::fail(53)
                    } else {
                        StepOutcome::Await
                    };
                }
                if io.send(PortId(1), self.live).is_err() {
                    return BrowserChatBack::fail(53);
                }
                self.pending = None;
                self.opened = true;
                StepOutcome::Progress
            }
            RECEIVE
                if outcome.disposition == HostCallDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return BrowserChatBack::fail(51);
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                let next_request = RequestId(self.next);
                let next_input = BoundedValueRef::new(
                    output.value,
                    conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
                )
                .expect("bounded receive continuation");
                if io.consume_host_completion().is_err()
                    || io.send(PortId(0), output.value).is_err()
                    || io
                        .request_host_call(next_request, RECEIVE, next_input)
                        .is_err()
                {
                    return BrowserChatBack::fail(53);
                }
                self.next = self.next.saturating_add(1);
                self.pending = Some((next_request, RECEIVE));
                StepOutcome::Progress
            }
            RECEIVE if outcome.disposition == HostCallDisposition::Cancelled => {
                if let Some(output) = outcome.output {
                    if !io.output_ready(PortId(1)) {
                        return StepOutcome::Await;
                    }
                    if io.consume_host_completion().is_err()
                        || io.send(PortId(1), output.value).is_err()
                    {
                        return BrowserChatBack::fail(53);
                    }
                } else if io.consume_host_completion().is_err() {
                    return BrowserChatBack::fail(53);
                }
                self.pending = None;
                StepOutcome::Progress
            }
            SEND if outcome.disposition == HostCallDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return BrowserChatBack::fail(52);
                };
                let next_request = RequestId(self.next);
                let next_input = BoundedValueRef::new(
                    output.value,
                    conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
                )
                .expect("bounded send continuation");
                if io.consume_host_completion().is_err()
                    || io
                        .request_host_call(next_request, RECEIVE, next_input)
                        .is_err()
                {
                    return BrowserChatBack::fail(53);
                }
                self.next = self.next.saturating_add(1);
                self.pending = Some((next_request, RECEIVE));
                StepOutcome::Progress
            }
            CLOSE
                if outcome.disposition == HostCallDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                if io.consume_host_completion().is_err() {
                    return BrowserChatBack::fail(53);
                }
                self.pending = None;
                StepOutcome::Complete
            }
            _ => BrowserChatBack::fail(53),
        }
    }
}
