use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{HostCallRequest, StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::BODY_CONVERSATION_CONTEXT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct BodyConversationContextOperation {
    token: Option<ValueRef>,
    pending: bool,
    emitted: bool,
    next_request: u32,
}

impl<const PORTS: usize> StepOperation<PORTS> for BodyConversationContextOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let Some(token) = self.token else {
            return step_fail(FailureCode::InvalidLifecycle, 1);
        };
        let input = BoundedValueRef::new(token, 0).expect("empty Body context source token");
        if let Some((request, outcome)) = io.host_completion() {
            let expected = self
                .next_request
                .checked_sub(1)
                .map(RequestId)
                .filter(|_| self.pending);
            if expected != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 5);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let request = RequestId(self.next_request);
                    let Some(next) = self.next_request.checked_add(1) else {
                        return step_fail(FailureCode::IdentityCapacityExhausted, 6);
                    };
                    io.consume_host_completion()
                        .expect("observed Body context source completion");
                    io.send(PortId(0), output.value)
                        .expect("ready Body context output");
                    io.request_host_call(request, HostCallId(0), input)
                        .expect("next Body context source Host Call");
                    self.next_request = next;
                    self.pending = true;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Denied, _, _) => {
                    return step_fail(FailureCode::HostCallDenied, 3)
                }
                (HostCallDisposition::Cancelled, None, None) => {
                    return step_fail(FailureCode::Cancelled, 4)
                }
                (_, _, Some(failure)) => return StepOutcome::Fail(failure),
                _ => return step_fail(FailureCode::HostCallFailed, 4),
            }
        }
        if self.pending {
            return StepOutcome::Await;
        }
        let request = RequestId(self.next_request);
        let Some(next) = self.next_request.checked_add(1) else {
            return step_fail(FailureCode::IdentityCapacityExhausted, 6);
        };
        io.request_host_call(request, HostCallId(0), input)
            .expect("Body context source Host Call");
        self.next_request = next;
        self.pending = true;
        StepOutcome::Progress
    }

    fn retains_host_call_input(&self, _request: RequestId, value: ValueRef) -> bool {
        self.token == Some(value)
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.token = None;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
impl BodyConversationContextOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        let Some(token) = self.token else {
            return fail(FailureCode::InvalidLifecycle, 1);
        };
        let Ok(input) = BoundedValueRef::new(token, 0) else {
            return fail(FailureCode::InvalidInput, 2);
        };
        self.request(input)
    }
    fn request(&mut self, input: BoundedValueRef) -> OperationAction {
        let request = RequestId(self.next_request);
        let Some(next_request) = self.next_request.checked_add(1) else {
            return fail(FailureCode::IdentityCapacityExhausted, 6);
        };
        self.next_request = next_request;
        self.pending = true;
        OperationAction::RequestHostCall {
            request,
            operation: HostCallId(0),
            input,
        }
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostCallCompleted {
                request: _,
                outcome,
            } if self.pending => {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostCallDisposition::Denied, _, _) => fail(FailureCode::HostCallDenied, 3),
                    (HostCallDisposition::Cancelled, None, None) => fail(FailureCode::Cancelled, 4),
                    _ => fail(FailureCode::HostCallFailed, 4),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 5),
        }
    }
    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            self.emitted = false;
            let token = self.token.expect("prepared Body context source token");
            self.request(BoundedValueRef::new(token, 0).expect("empty source token"))
        } else {
            OperationAction::Await
        }
    }
    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

pub(super) struct BodyConversationContextHost<'a> {
    source: Option<&'a crate::BodyConversationContextSource>,
    pending: Option<HostCallRequest>,
    delivered: Option<[u8; 32]>,
}

impl<'a> BodyConversationContextHost<'a> {
    pub(super) fn new(source: Option<&'a crate::BodyConversationContextSource>) -> Self {
        Self {
            source,
            pending: None,
            delivered: None,
        }
    }

    pub(super) fn accept(&mut self, request: HostCallRequest, input: &[u8]) -> Result<(), String> {
        if !input.is_empty() {
            return Err("Body context source request carries unexpected bytes".into());
        }
        if self.source.is_none() {
            return Err("Body context was planned without a supervisor source".into());
        }
        if self.pending.replace(request).is_some() {
            return Err("Body context source has two pending requests".into());
        }
        Ok(())
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(super) fn poll(
        &mut self,
        scheduler: &mut super::InstalledScheduler,
    ) -> Result<bool, String> {
        let Some(request) = self.pending else {
            return Ok(false);
        };
        let source = self
            .source
            .ok_or_else(|| "pending Body context request lost its source".to_string())?;
        let poll = source.poll_after(self.delivered, |encoded| {
            scheduler.store_host_value(encoded)
        });
        let outcome = match poll {
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Pending => {
                return Ok(false)
            }
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Lost => {
                HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                }
            }
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Current {
                fingerprint,
                value,
            } => {
                let value =
                    value.map_err(|error| format!("store current body context: {error:?}"))?;
                self.delivered = Some(fingerprint);
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(
                            value,
                            conduit_body::MAXIMUM_BODY_CONVERSATION_CONTEXT_BYTES as u32,
                        )
                        .map_err(|error| format!("bound current body context: {error:?}"))?,
                    ),
                    failure: None,
                }
            }
        };
        self.pending = None;
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(|error| format!("complete current body context: {error:?}"))?;
        Ok(true)
    }
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::body_conversation_context_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::BODY_CONVERSATION_CONTEXT_STD_PROFILE
        || placement.artifact_id.as_str()
            != conduit_std_offers::BODY_CONVERSATION_CONTEXT_STD_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
    {
        return Err(
            "planned body conversation context identity does not match installation".into(),
        );
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: conduit_body::MAXIMUM_BODY_CONVERSATION_CONTEXT_BYTES as u32,
        host_requests: 1,
        sign_items: 8,
        maximum_value_bytes: conduit_body::MAXIMUM_BODY_CONVERSATION_CONTEXT_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let token = values
        .store(&[])
        .map_err(|error| format!("store Body context source token: {error:?}"))?;
    Ok(InstalledOperation::BodyConversationContext(
        BodyConversationContextOperation {
            token: Some(token),
            pending: false,
            emitted: false,
            next_request: 0,
        },
    ))
}
fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(byte_len: u32) -> ValueRef {
        ValueRef {
            slot: 0,
            generation: 0,
            byte_len,
        }
    }

    #[test]
    fn current_source_requests_each_replacement_and_never_completes_after_first_value() {
        let mut operation = BodyConversationContextOperation {
            token: Some(value(0)),
            pending: false,
            emitted: false,
            next_request: 0,
        };
        assert!(matches!(
            operation.start(),
            OperationAction::RequestHostCall {
                request: RequestId(0),
                ..
            }
        ));
        let output = BoundedValueRef::new(value(12), 12).unwrap();
        assert!(matches!(
            operation.resume(OperationInput::HostCallCompleted {
                request: RequestId(0),
                outcome: HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            }),
            OperationAction::Emit {
                port: PortId(0),
                ..
            }
        ));
        assert!(matches!(
            operation.advance(),
            OperationAction::RequestHostCall {
                request: RequestId(1),
                ..
            }
        ));
    }
}
