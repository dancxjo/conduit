use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::BODY_CHAT_PROMPT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct BodyChatPromptOperation {
    pending: Option<RequestId>,
    next_request: u32,
    queued_human: Option<ValueRef>,
    emit_human: bool,
    closed: [bool; 3],
}

impl<const PORTS: usize> StepOperation<PORTS> for BodyChatPromptOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 4);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if self.queued_human.is_some() {
                        if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
                            return StepOutcome::Await;
                        }
                        let human = self.queued_human.expect("queued human message");
                        io.consume_host_completion()
                            .expect("observed Body Chat prompt completion");
                        io.send(PortId(0), output.value)
                            .expect("ready Body Chat prompt output");
                        io.send(PortId(1), human)
                            .expect("ready Body Chat human-message output");
                        self.queued_human = None;
                    } else {
                        if !io.output_ready(PortId(2)) {
                            return StepOutcome::Await;
                        }
                        io.consume_host_completion()
                            .expect("observed Body Chat response completion");
                        io.send(PortId(2), output.value)
                            .expect("ready Body Chat response output");
                    }
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed Body Chat state completion");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Denied, _, _) => {
                    return step_fail(FailureCode::HostCallDenied, 2)
                }
                (_, _, Some(failure)) => return StepOutcome::Fail(failure),
                _ => return step_fail(FailureCode::HostCallFailed, 3),
            }
        }
        for port in 0..3_u16 {
            let id = PortId(port);
            let Some(value) = io.input(id) else {
                continue;
            };
            let maximum = if port == 2 {
                conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES
            } else {
                conduit_chat::MAXIMUM_BODY_CHAT_MESSAGE_BYTES
            } as u32;
            let Ok(input) = BoundedValueRef::new(value, maximum) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::IdentityCapacityExhausted, 6);
            };
            if port == 0 {
                self.queued_human =
                    Some(io.take_input(id).expect("present Body Chat human message"));
            } else {
                io.consume(id).expect("present Body Chat state input");
            }
            io.request_host_call(request, HostCallId(port), input)
                .expect("Body Chat prompt Host Call");
            self.next_request = next;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        for port in 0..3_u16 {
            let id = PortId(port);
            if io.input_closed(id) && !self.closed[usize::from(port)] {
                io.consume_closed(id)
                    .expect("observed Body Chat input closure");
                self.closed[usize::from(port)] = true;
                return StepOutcome::Progress;
            }
        }
        StepOutcome::Await
    }

    fn retains_host_call_input(&self, _request: RequestId, value: ValueRef) -> bool {
        self.queued_human == Some(value)
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.queued_human = None;
        self.emit_human = false;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl BodyChatPromptOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(port),
                value,
            } if port < 3 && self.pending.is_none() => {
                if port == 0 {
                    self.queued_human = Some(value);
                }
                let maximum = if port == 2 {
                    conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES
                } else {
                    conduit_chat::MAXIMUM_BODY_CHAT_MESSAGE_BYTES
                } as u32;
                let Ok(input) = BoundedValueRef::new(value, maximum) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                self.request(port, input)
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => {
                        let port = if self.queued_human.is_some() {
                            self.emit_human = true;
                            0
                        } else {
                            2
                        };
                        OperationAction::Emit {
                            port: PortId(port),
                            value: output.value,
                        }
                    }
                    (HostCallDisposition::Completed, None, None) => OperationAction::Await,
                    (HostCallDisposition::Denied, _, _) => fail(FailureCode::HostCallDenied, 2),
                    _ => fail(FailureCode::HostCallFailed, 3),
                }
            }
            OperationInput::Closed { port: PortId(port) } if port < 3 => OperationAction::Await,
            _ => fail(FailureCode::InvalidLifecycle, 4),
        }
    }
    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emit_human {
            self.emit_human = false;
            if let Some(value) = self.queued_human.take() {
                return OperationAction::Emit {
                    port: PortId(1),
                    value,
                };
            }
        }
        OperationAction::Await
    }
    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.queued_human = None;
        self.emit_human = false;
    }

    fn request(&mut self, port: u16, input: BoundedValueRef) -> OperationAction {
        let request = RequestId(self.next_request);
        let Some(next_request) = self.next_request.checked_add(1) else {
            return fail(FailureCode::IdentityCapacityExhausted, 6);
        };
        self.next_request = next_request;
        self.pending = Some(request);
        OperationAction::RequestHostCall {
            request,
            operation: HostCallId(port),
            input,
        }
    }
}

pub(super) struct BodyChatPromptHost {
    state: Option<conduit_chat::BodyChatPromptState>,
    output: Vec<u8>,
}

impl BodyChatPromptHost {
    fn new() -> Self {
        Self {
            state: None,
            output: Vec::with_capacity(conduit_chat::MAXIMUM_BODY_CHAT_PROMPT_BYTES),
        }
    }
    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<&[u8]>, String> {
        match contract {
            conduit_std_offers::BODY_CHAT_CONTEXT_OPERATION => {
                if let Some(state) = &mut self.state {
                    state.replace_context(input)
                } else {
                    self.state = Some(
                        conduit_chat::BodyChatPromptState::new(
                            input,
                            conduit_chat::MAXIMUM_BODY_CHAT_HISTORY_ITEMS,
                        )
                        .map_err(|error| format!("Body Chat context: {error:?}"))?,
                    );
                    Ok(())
                }
                .map_err(|error| format!("Body Chat context: {error:?}"))?;
                Ok(None)
            }
            conduit_std_offers::BODY_CHAT_MESSAGE_OPERATION => {
                let request = self
                    .state
                    .as_mut()
                    .ok_or_else(|| {
                        "Body Chat message arrived before current body context".to_string()
                    })?
                    .request(input)
                    .map_err(|error| format!("Body Chat message: {error:?}"))?;
                self.output.clear();
                self.output.extend_from_slice(&request.encoded_request);
                Ok(Some(&self.output))
            }
            conduit_std_offers::BODY_CHAT_RESPONSE_OPERATION => {
                self.state
                    .as_mut()
                    .ok_or_else(|| {
                        "Body Chat response arrived before current body context".to_string()
                    })?
                    .record_response(input)
                    .map_err(|error| format!("Body Chat response: {error:?}"))?;
                self.output.clear();
                self.output.extend_from_slice(input);
                Ok(Some(&self.output))
            }
            _ => Err("unknown Body Chat prompt operation".into()),
        }
    }
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Vec<Option<BodyChatPromptHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::BODY_CHAT_PROMPT_STD_IMPLEMENTATION)
                .then(BodyChatPromptHost::new)
        })
        .collect()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::body_chat_prompt_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::BODY_CHAT_PROMPT_STD_PROFILE
        || placement.artifact_id.as_str() != conduit_std_offers::BODY_CHAT_PROMPT_STD_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
    {
        return Err("planned body Chat prompt identity does not match installation".into());
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: (conduit_chat::MAXIMUM_BODY_CHAT_PROMPT_BYTES
            + conduit_chat::MAXIMUM_BODY_CHAT_MESSAGE_BYTES) as u32,
        host_requests: 3,
        sign_items: 32,
        maximum_value_bytes: conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::BodyChatPrompt(
        BodyChatPromptOperation {
            pending: None,
            next_request: 0,
            queued_human: None,
            emit_human: false,
            closed: [false; 3],
        },
    ))
}
fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        Body, BodyConversationContext, BodyConversationContextBasis, BodyConversationHost,
    };
    use conduit_core::{CheckedFormId, HostId, SignId, SourceDocumentId};

    fn context() -> Vec<u8> {
        let body = Body::born(
            SourceDocumentId::from("source/body-chat"),
            CheckedFormId::from("checked/body-chat"),
            1,
            SignId::from("sign/born"),
        )
        .unwrap();
        let (_, wake) = body.wake(1, SignId::from("sign/wake")).unwrap();
        conduit_chat::encode_body_conversation_context(&BodyConversationContext {
            schema: "conduit.body/conversation-context-value@2".into(),
            display_name: "Roseau".into(),
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            wake_sequence: 1,
            basis: BodyConversationContextBasis {
                body_id: wake.body_id.clone(),
                wake_id: wake.wake_id.clone(),
                wake_sequence: 1,
                revision: 0,
            },
            hosts: vec![BodyConversationHost {
                host_id: HostId::from("Latimer"),
                present: true,
            }],
            active_forms: vec!["Tour".into()],
            current_plan_id: None,
            active_play_id: None,
            lines: vec![],
            recent_sign_ids: vec![],
        })
        .unwrap()
    }

    #[test]
    fn installed_prompt_host_owns_bounded_history_across_stateless_model_requests() {
        let mut host = BodyChatPromptHost::new();
        assert!(host
            .execute(conduit_std_offers::BODY_CHAT_CONTEXT_OPERATION, &context())
            .unwrap()
            .is_none());
        let first = host
            .execute(
                conduit_std_offers::BODY_CHAT_MESSAGE_OPERATION,
                b"What are you doing?",
            )
            .unwrap()
            .unwrap()
            .to_vec();
        assert!(core::str::from_utf8(&first)
            .unwrap()
            .contains("\"active_forms\":1"));
        host.execute(
            conduit_std_offers::BODY_CHAT_RESPONSE_OPERATION,
            b"I am running the Tour.",
        )
        .unwrap();
        let second = host
            .execute(
                conduit_std_offers::BODY_CHAT_MESSAGE_OPERATION,
                b"Which host is present?",
            )
            .unwrap()
            .unwrap();
        let second = core::str::from_utf8(second).unwrap();
        assert!(second.contains("I am running the Tour."));
        assert!(second.contains("\"present_hosts\":1"));
        assert!(!second.contains("Latimer"));
    }
}
