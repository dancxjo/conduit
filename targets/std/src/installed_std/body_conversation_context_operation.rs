use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
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
}
impl BodyConversationContextOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        let Some(token) = self.token.take() else {
            return fail(FailureCode::InvalidLifecycle, 1);
        };
        let Ok(input) = BoundedValueRef::new(token, 0) else {
            return fail(FailureCode::InvalidInput, 2);
        };
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: HostOperationId(0),
            input,
        }
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
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
                    _ => fail(FailureCode::HostOperationFailed, 4),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 5),
        }
    }
    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
    pub(super) fn cancel(&mut self) {
        self.pending = false;
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
            "planned Body conversation context identity does not match installation".into(),
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
        },
    ))
}
fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
