use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection, PortTemporal, BOOL_ENCODED_LEN};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) static BOOL_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::BOOL_PRESENTATION_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct BoolPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
    maximum: u64,
}

impl BoolPresentationOperation {
    pub(super) fn new(maximum: u64) -> Self {
        Self {
            pending: None,
            next: 0,
            maximum,
        }
    }
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && u64::from(self.next) < self.maximum => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, BOOL_ENCODED_LEN as u32) else {
                    return InstalledOperation::fail(47);
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
                self.next = self.next.saturating_add(1);
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(47),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::bool_presentation_offer();
    if placement.kind_id.as_str() != conduit_semantic_catalog::BOOL_PRESENTATION_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::BOOL_PRESENTATION_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::BOOL_PRESENTATION_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::BOOL_PRESENTATION_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::BOOL_PRESENTATION_ARTIFACT
        || placement.inputs != offer.inputs
        || !placement.outputs.is_empty()
        || placement.inputs[0].port_id.as_str() != "value"
        || placement.inputs[0].direction != PortDirection::Input
        || placement.inputs[0].temporal != PortTemporal::Current
        || placement.host_operations != offer.host_operations
        || placement.resources.len() != 1
    {
        return Err("planned Boolean presentation identity does not match its installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: conduit_semantic_catalog::MAX_TOGGLE_VALUES as usize,
        sign_items: 64,
        maximum_value_bytes: BOOL_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::BoolPresentation(
        BoolPresentationOperation {
            pending: None,
            next: 0,
            maximum: conduit_semantic_catalog::MAX_TOGGLE_VALUES,
        },
    ))
}
