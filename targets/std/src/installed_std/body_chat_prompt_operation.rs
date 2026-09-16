use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
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
                    conduit_tongues::MAXIMUM_BODY_CHAT_CONTEXT_BYTES
                } else {
                    conduit_tongues::MAXIMUM_BODY_CHAT_MESSAGE_BYTES
                } as u32;
                let Ok(input) = BoundedValueRef::new(value, maximum) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(port),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
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
                    (HostOperationDisposition::Completed, None, None) => OperationAction::Await,
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 2)
                    }
                    _ => fail(FailureCode::HostOperationFailed, 3),
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
}

pub(super) struct BodyChatPromptHost {
    state: Option<conduit_tongues::BodyChatPromptState>,
    output: Vec<u8>,
}

impl BodyChatPromptHost {
    fn new() -> Self {
        Self {
            state: None,
            output: Vec::with_capacity(conduit_tongues::MAXIMUM_BODY_CHAT_PROMPT_BYTES),
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
                        conduit_tongues::BodyChatPromptState::new(
                            input,
                            conduit_tongues::MAXIMUM_BODY_CHAT_HISTORY_ITEMS,
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
                        "Body Chat message arrived before current Body context".to_string()
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
                        "Body Chat response arrived before current Body context".to_string()
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
        || placement.host_operations != offer.host_operations
    {
        return Err("planned Body Chat prompt identity does not match installation".into());
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: (conduit_tongues::MAXIMUM_BODY_CHAT_PROMPT_BYTES
            + conduit_tongues::MAXIMUM_BODY_CHAT_MESSAGE_BYTES) as u32,
        host_requests: 3,
        sign_items: 32,
        maximum_value_bytes: conduit_tongues::MAXIMUM_BODY_CHAT_CONTEXT_BYTES as u32,
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
        },
    ))
}
fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{Body, BodyConversationContext, BodyConversationHost};
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
        conduit_tongues::encode_body_conversation_context(&BodyConversationContext {
            schema: "conduit.body/conversation-context-value@1".into(),
            display_name: "Roseau".into(),
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id,
            wake_sequence: 1,
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
        assert!(core::str::from_utf8(&first).unwrap().contains("Tour"));
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
        assert!(second.contains("Latimer"));
    }
}
