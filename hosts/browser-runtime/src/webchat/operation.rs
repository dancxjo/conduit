use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

const CLOSE: HostOperationId = HostOperationId(0);
const OPEN: HostOperationId = HostOperationId(1);
const RECEIVE: HostOperationId = HostOperationId(2);
const SEND: HostOperationId = HostOperationId(3);

pub(crate) enum BrowserChatOperation {
    State(State),
    Tee,
    Renderer(Request),
    Interaction(Interaction),
    Submit(Request),
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
    pending: Option<(RequestId, HostOperationId)>,
    after_receive: Option<ValueRef>,
    opened: bool,
}

impl BrowserChatOperation {
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
            after_receive: None,
            opened: false,
        })
    }
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl Operation for BrowserChatOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::State(value) => value.initial.take().map_or_else(
                || Self::fail(40),
                |value| OperationAction::Emit {
                    port: PortId(0),
                    value,
                },
            ),
            Self::Socket(value) => value.request_open(),
            _ => OperationAction::Await,
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::State(value) => value.resume(input),
            Self::Tee => match input {
                OperationInput::Value {
                    port: PortId(0),
                    value,
                } => OperationAction::Emit {
                    port: PortId(0),
                    value,
                },
                OperationInput::Closed { port: PortId(0) } => OperationAction::Complete,
                _ => Self::fail(41),
            },
            Self::Renderer(value) | Self::Submit(value) => value.resume(input),
            Self::Interaction(value) => value.resume(input),
            Self::Socket(value) => value.resume(input),
        }
    }
    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Socket(value) => value.advance(),
            _ => OperationAction::Await,
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::State(value) => value.pending = None,
            Self::Renderer(value) | Self::Submit(value) => value.pending = None,
            Self::Interaction(value) => value.pending = None,
            Self::Socket(value) => value.pending = None,
            Self::Tee => {}
        }
    }
}

impl State {
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value } if self.pending.is_none() && port.0 < 2 => {
                let request = RequestId(self.next);
                self.next = self.next.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(1 - port.0),
                    input: BoundedValueRef::new(
                        value,
                        if port.0 == 0 {
                            conduit_chat::MAXIMUM_CHAT_MESSAGE_BYTES
                        } else {
                            1
                        },
                    )
                    .expect("bounded state input"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending.take() == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                outcome.output.map_or_else(
                    || BrowserChatOperation::fail(42),
                    |output| OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    },
                )
            }
            _ => BrowserChatOperation::fail(43),
        }
    }
}

impl Request {
    fn new(maximum: u32) -> Self {
        Self {
            pending: None,
            next: 0,
            maximum,
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let request = RequestId(self.next);
                self.next = self.next.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, self.maximum)
                        .expect("bounded operation input"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending.take() == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                outcome
                    .output
                    .map_or(OperationAction::Await, |output| OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    })
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => BrowserChatOperation::fail(44),
        }
    }
}

impl Interaction {
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                self.presentation = Some(value);
                self.request_if_ready()
            }
            OperationInput::Value {
                port: PortId(1),
                value,
            } if self.pending.is_none() => {
                self.manifestation = Some(value);
                self.request_if_ready()
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending.take() == Some(request) =>
            {
                if outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none()
                {
                    outcome
                        .output
                        .map_or(OperationAction::Await, |output| OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        })
                } else if outcome.disposition == HostOperationDisposition::Cancelled {
                    OperationAction::Await
                } else {
                    BrowserChatOperation::fail(45)
                }
            }
            _ => BrowserChatOperation::fail(46),
        }
    }
    fn request_if_ready(&mut self) -> OperationAction {
        let (Some(_), Some(_)) = (self.presentation, self.manifestation) else {
            return OperationAction::Await;
        };
        let request = RequestId(self.next);
        self.next = self.next.saturating_add(1);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.token, 0).expect("empty admitted input token"),
        }
    }
}

impl Socket {
    fn request_open(&mut self) -> OperationAction {
        let Some(value) = self.open.take() else {
            return BrowserChatOperation::fail(47);
        };
        self.request(OPEN, value, 256)
    }
    fn request(
        &mut self,
        operation: HostOperationId,
        value: ValueRef,
        maximum: u32,
    ) -> OperationAction {
        let request = RequestId(self.next);
        self.next = self.next.saturating_add(1);
        self.pending = Some((request, operation));
        OperationAction::RequestHostOperation {
            request,
            operation,
            input: BoundedValueRef::new(value, maximum).expect("bounded socket input"),
        }
    }
    fn advance(&mut self) -> OperationAction {
        if self.opened {
            self.opened = false;
            let value = self.receive.take().unwrap_or(self.live);
            return self.request(
                RECEIVE,
                value,
                conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
            );
        }
        let Some(value) = self.after_receive.take() else {
            return OperationAction::Await;
        };
        self.request(
            RECEIVE,
            value,
            conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
        )
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => self.request(
                SEND,
                value,
                conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
            ),
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                let Some(value) = self.close.take() else {
                    return BrowserChatOperation::fail(48);
                };
                self.request(CLOSE, value, 1)
            }
            OperationInput::HostOperationCompleted { request, outcome } => {
                let Some((expected, operation)) = self.pending.take() else {
                    return BrowserChatOperation::fail(49);
                };
                if request != expected {
                    return BrowserChatOperation::fail(50);
                }
                match operation {
                    OPEN if outcome.disposition == HostOperationDisposition::Completed
                        && outcome.failure.is_none() =>
                    {
                        self.opened = true;
                        OperationAction::Emit {
                            port: PortId(1),
                            value: self.live,
                        }
                    }
                    RECEIVE
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.failure.is_none() =>
                    {
                        let Some(output) = outcome.output else {
                            return BrowserChatOperation::fail(51);
                        };
                        self.after_receive = Some(output.value);
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    RECEIVE if outcome.disposition == HostOperationDisposition::Cancelled => {
                        outcome.output.map_or(OperationAction::Await, |output| {
                            OperationAction::Emit {
                                port: PortId(1),
                                value: output.value,
                            }
                        })
                    }
                    SEND if outcome.disposition == HostOperationDisposition::Completed
                        && outcome.failure.is_none() =>
                    {
                        let Some(output) = outcome.output else {
                            return BrowserChatOperation::fail(52);
                        };
                        self.request(
                            RECEIVE,
                            output.value,
                            conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
                        )
                    }
                    CLOSE
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.failure.is_none() =>
                    {
                        OperationAction::Complete
                    }
                    _ => BrowserChatOperation::fail(53),
                }
            }
            _ => BrowserChatOperation::fail(54),
        }
    }
}
