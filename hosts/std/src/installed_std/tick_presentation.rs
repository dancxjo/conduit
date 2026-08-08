use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedOperation, PortDirection};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) static TICK_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::TICK_PRESENTATION_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TickPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
    maximum_values: u32,
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
            } if self.pending.is_none() && self.next < self.maximum_values => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, conduit_std_catalog::TICK_ENCODED_LEN)
                else {
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
                self.next = self.next.saturating_add(1);
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

fn maximum_values(placement: &PlannedOperation) -> Result<u64, String> {
    if placement.configuration.len() != 1 {
        return Err(
            "presentation/tick requires exactly one planned configuration field".to_string(),
        );
    }
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            ("maximum-values", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .filter(|value| (1..=conduit_std_catalog::TIME_EVERY_COUNT).contains(value))
        .ok_or_else(|| "presentation/tick maximum-values is missing or invalid".to_string())
}

fn validate(placement: &PlannedOperation) -> Result<(), String> {
    if placement.kind_id.as_str() != conduit_std_catalog::TICK_PRESENTATION_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_std_catalog::TICK_PRESENTATION_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_catalog::TICK_PRESENTATION_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_catalog::TICK_PRESENTATION_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_catalog::TICK_PRESENTATION_ARTIFACT
        || placement.inputs.len() != 1
        || !placement.outputs.is_empty()
        || placement.inputs[0].port_id.as_str() != "tick"
        || placement.inputs[0].value_kind.as_str() != conduit_std_catalog::TICK_VALUE_KIND
        || placement.inputs[0].direction != PortDirection::Input
    {
        return Err(
            "planned tick presentation identity does not match its installation".to_string(),
        );
    }
    maximum_values(placement).map(|_| ())
}

fn budget(placement: &PlannedOperation) -> Result<OperationBudget, String> {
    validate(placement)?;
    let maximum = maximum_values(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: maximum as usize,
        evidence_items: 64,
        maximum_value_bytes: conduit_std_catalog::TICK_ENCODED_LEN,
    })
}

fn prepare(
    placement: &PlannedOperation,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::TickPresentation(
        TickPresentationOperation {
            pending: None,
            next: 0,
            maximum_values: maximum_values(placement)? as u32,
        },
    ))
}
