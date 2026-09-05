//! Kernel lifecycle for bounded named-pattern storage commands.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TEMPLATE_STORAGE_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TemplateStorageOperation {
    pending: Option<RequestId>,
    next_request: u32,
    completed_commands: u64,
    maximum_commands: u64,
    closed: bool,
}

impl TemplateStorageOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none()
                && !self.closed
                && self.completed_commands < self.maximum_commands =>
            {
                let request = RequestId(self.next_request);
                self.next_request = match self.next_request.checked_add(1) {
                    Some(next) => next,
                    None => return fail(FailureCode::StorageExhausted, 263),
                };
                self.pending = Some(request);
                let Ok(input) =
                    BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
                else {
                    self.pending = None;
                    return fail(FailureCode::InvalidInput, 264);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.completed_commands += 1;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 0),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 260),
                }
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && !self.closed =>
            {
                self.closed = true;
                OperationAction::Complete
            }
            OperationInput::Value {
                port: PortId(0), ..
            } if self.completed_commands >= self.maximum_commands => {
                fail(FailureCode::StorageExhausted, 262)
            }
            _ => fail(FailureCode::InvalidLifecycle, 261),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.closed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

pub(super) fn refusal_detail(refusal: super::template_storage_host::StorageRefusal) -> u16 {
    use super::template_storage_host::StorageRefusal::*;
    match refusal {
        Malformed => 1,
        DuplicateName => 2,
        Full => 3,
        CorruptRetainedTemplate => 4,
    }
}

fn validate(placement: &PlannedGear) -> Result<u64, String> {
    let offer = conduit_std_offers::template_storage_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
    {
        return Err("planned template storage differs from installed realization".into());
    }
    if placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_std_offers::TEMPLATE_STORAGE_RESOURCE_CLASS
        || placement.resources[0].units != 1
    {
        return Err("planned template storage lacks its exact admitted slot".into());
    }
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-commands", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("template storage command bound is absent")?;
    if !(1..=conduit_semantic_catalog::MAXIMUM_TEMPLATE_STORAGE_COMMANDS).contains(&maximum) {
        return Err("template storage command bound is outside reviewed limits".into());
    }
    Ok(maximum)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let maximum = validate(placement)?;
    Ok(OperationBudget {
        value_items: u16::try_from(maximum + 1).map_err(|_| "template value budget overflow")?,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32).saturating_mul(maximum as u32 + 1),
        host_requests: usize::try_from(maximum).map_err(|_| "template request budget overflow")?,
        sign_items: u16::try_from(maximum.saturating_mul(8))
            .map_err(|_| "template sign budget overflow")?,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let maximum_commands = validate(placement)?;
    Ok(InstalledOperation::TemplateStorage(
        TemplateStorageOperation {
            pending: None,
            next_request: 0,
            completed_commands: 0,
            maximum_commands,
            closed: false,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
