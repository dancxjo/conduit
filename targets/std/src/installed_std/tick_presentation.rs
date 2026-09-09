use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static TICK_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TICK_PRESENTATION_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TickPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
}

impl TickPresentationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, conduit_time::TICK_ENCODED_LEN) else {
                    return InstalledOperation::fail(9);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                let Some(next) = self.next.checked_add(1) else {
                    return OperationAction::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 9,
                    });
                };
                self.next = next;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(9),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != conduit_semantic_catalog::TICK_PRESENTATION_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::TICK_PRESENTATION_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::TICK_PRESENTATION_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::TICK_PRESENTATION_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::TICK_PRESENTATION_ARTIFACT
        || placement.inputs.len() != 1
        || !placement.outputs.is_empty()
        || placement.inputs[0].port_id.as_str() != "tick"
        || placement.inputs[0].value_kind.as_str() != conduit_time::TICK_VALUE_KIND
        || placement.inputs[0].direction != PortDirection::Input
        || !placement.configuration.is_empty()
    {
        return Err(
            "planned tick presentation identity does not match its installation".to_string(),
        );
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 64,
        maximum_value_bytes: conduit_time::TICK_ENCODED_LEN,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::TickPresentation(
        TickPresentationOperation {
            pending: None,
            next: 0,
        },
    ))
}
