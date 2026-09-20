use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::HostOperationRequest, BoundedValueRef, Failure, FailureCode,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, OperationAction,
    OperationInput, PortId, RequestId, ValueRef, ValueStorage,
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
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input,
        }
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted {
                request: _,
                outcome,
            } if self.pending => {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 3)
                    }
                    (HostOperationDisposition::Cancelled, None, None) => {
                        fail(FailureCode::Cancelled, 4)
                    }
                    _ => fail(FailureCode::HostOperationFailed, 4),
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
    pending: Option<HostOperationRequest>,
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

    pub(super) fn accept(
        &mut self,
        request: HostOperationRequest,
        input: &[u8],
    ) -> Result<(), String> {
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
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Cancelled,
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
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
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
            .complete_host_operation(request.node, request.request, outcome)
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
        || placement.host_operations != offer.host_operations
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
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                ..
            }
        ));
        let output = BoundedValueRef::new(value(12), 12).unwrap();
        assert!(matches!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
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
            OperationAction::RequestHostOperation {
                request: RequestId(1),
                ..
            }
        ));
    }
}
