use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

const SOCKET_CLOSE: HostOperationId = HostOperationId(0);
const SOCKET_OPEN: HostOperationId = HostOperationId(1);
const SOCKET_RECEIVE: HostOperationId = HostOperationId(2);
const SOCKET_SEND: HostOperationId = HostOperationId(3);

pub(crate) enum BrowserChatOperation {
    TextInput(TextInputOperation),
    Socket(SocketOperation),
    List(ListOperation),
}

pub(crate) struct TextInputOperation {
    tokens: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

pub(crate) struct SocketOperation {
    open: Option<ValueRef>,
    receive: Option<ValueRef>,
    close: Option<ValueRef>,
    next_request: u32,
    pending: Option<(RequestId, HostOperationId)>,
    after_receive: Option<ValueRef>,
}

pub(crate) struct ListOperation {
    next_request: u32,
    pending: Option<RequestId>,
    retained: u16,
}

impl BrowserChatOperation {
    pub(crate) fn text_input(tokens: Vec<ValueRef>) -> Self {
        Self::TextInput(TextInputOperation {
            tokens,
            next: 0,
            pending: None,
        })
    }

    pub(crate) fn socket(open: ValueRef, receive: ValueRef, close: ValueRef) -> Self {
        Self::Socket(SocketOperation {
            open: Some(open),
            receive: Some(receive),
            close: Some(close),
            next_request: 0,
            pending: None,
            after_receive: None,
        })
    }

    pub(crate) fn list() -> Self {
        Self::List(ListOperation {
            next_request: 0,
            pending: None,
            retained: 0,
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
            Self::TextInput(operation) => operation.request(),
            Self::Socket(operation) => operation.request_open(),
            Self::List(_) => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::TextInput(operation) => operation.resume(input),
            Self::Socket(operation) => operation.resume(input),
            Self::List(operation) => operation.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::TextInput(operation) => operation.request(),
            Self::Socket(operation) => operation.request_after_receive(),
            Self::List(_) => OperationAction::Await,
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::TextInput(operation) => operation.pending = None,
            Self::Socket(operation) => operation.pending = None,
            Self::List(operation) => operation.pending = None,
        }
    }
}

impl TextInputOperation {
    fn request(&mut self) -> OperationAction {
        let Some(token) = self.tokens.get(self.next).copied() else {
            return OperationAction::Complete;
        };
        let request = RequestId(self.next as u32);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(token, 1).expect("text-input token is one byte"),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        let OperationInput::HostOperationCompleted { request, outcome } = input else {
            return BrowserChatOperation::fail(41);
        };
        if self.pending.take() != Some(request) {
            return BrowserChatOperation::fail(42);
        }
        match (outcome.disposition, outcome.output, outcome.failure) {
            (HostOperationDisposition::Completed, Some(output), None) => {
                self.next += 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (HostOperationDisposition::Cancelled, None, _) => OperationAction::Complete,
            _ => BrowserChatOperation::fail(43),
        }
    }
}

impl SocketOperation {
    fn request_open(&mut self) -> OperationAction {
        let Some(value) = self.open.take() else {
            return BrowserChatOperation::fail(44);
        };
        self.request(SOCKET_OPEN, value, 256)
    }

    fn request_receive(&mut self, value: ValueRef) -> OperationAction {
        self.request(
            SOCKET_RECEIVE,
            value,
            conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
        )
    }

    fn request_after_receive(&mut self) -> OperationAction {
        let Some(value) = self.after_receive.take() else {
            return BrowserChatOperation::fail(45);
        };
        self.request_receive(value)
    }

    fn request(
        &mut self,
        operation: HostOperationId,
        value: ValueRef,
        maximum: u32,
    ) -> OperationAction {
        let request = RequestId(self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        self.pending = Some((request, operation));
        OperationAction::RequestHostOperation {
            request,
            operation,
            input: BoundedValueRef::new(value, maximum).expect("socket host input is bounded"),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => self.request(
                SOCKET_SEND,
                value,
                conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
            ),
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                let Some(value) = self.close.take() else {
                    return BrowserChatOperation::fail(46);
                };
                self.request(SOCKET_CLOSE, value, 1)
            }
            OperationInput::HostOperationCompleted { request, outcome } => {
                let Some((expected, operation)) = self.pending.take() else {
                    return BrowserChatOperation::fail(47);
                };
                if request != expected {
                    return BrowserChatOperation::fail(48);
                }
                match operation {
                    SOCKET_OPEN
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.failure.is_none() =>
                    {
                        let Some(value) = self.receive.take() else {
                            return BrowserChatOperation::fail(49);
                        };
                        self.request_receive(value)
                    }
                    SOCKET_RECEIVE
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.failure.is_none() =>
                    {
                        let Some(output) = outcome.output else {
                            return BrowserChatOperation::fail(50);
                        };
                        self.after_receive = Some(output.value);
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    SOCKET_RECEIVE
                        if outcome.disposition == HostOperationDisposition::Cancelled =>
                    {
                        if outcome.failure.is_some_and(|failure| failure.detail == 2) {
                            OperationAction::Complete
                        } else if outcome.output.is_none() {
                            OperationAction::Await
                        } else {
                            BrowserChatOperation::fail(51)
                        }
                    }
                    SOCKET_SEND
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.failure.is_none() =>
                    {
                        let Some(output) = outcome.output else {
                            return BrowserChatOperation::fail(52);
                        };
                        self.request_receive(output.value)
                    }
                    SOCKET_CLOSE
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.output.is_none()
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

impl ListOperation {
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none()
                && self.retained < conduit_chat::MAXIMUM_CHAT_HISTORY_ITEMS =>
            {
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(
                        value,
                        conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
                    )
                    .expect("chat message is bounded"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending.take() == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.retained += 1;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => BrowserChatOperation::fail(55),
        }
    }
}
