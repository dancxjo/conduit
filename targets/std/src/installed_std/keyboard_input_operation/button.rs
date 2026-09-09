//! Standing Space-key to semantic button implementation.
pub(in crate::installed_std) mod indicator;
#[cfg(test)]
mod tests;
use super::super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PreparedStructuredValueValidator};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedValueStore, OperationAction, PortId, RequestId, ValueRef,
    ValueStorage,
};

pub(crate) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::button::IMPLEMENTATION,
    budget,
    prepare,
};

pub(crate) struct ButtonOperation {
    empty: ValueRef,
    empty_released: bool,
    next: u32,
    pending: Option<RequestId>,
    terminal: bool,
    validator: PreparedStructuredValueValidator,
}

impl ButtonOperation {
    pub(crate) fn take_released_value(&mut self) -> Option<ValueRef> {
        if self.terminal && !self.empty_released {
            self.empty_released = true;
            return Some(self.empty);
        }
        None
    }
    pub(crate) fn start(&mut self) -> OperationAction {
        self.request()
    }
    pub(crate) fn advance(&mut self) -> OperationAction {
        self.request()
    }
    pub(crate) fn cancel(&mut self) {
        self.pending = None;
        self.terminal = true;
    }
    pub(crate) fn allocation_capacity(&self) -> usize {
        0
    }

    fn request(&mut self) -> OperationAction {
        if self.terminal || self.pending.is_some() {
            return fail(FailureCode::InvalidLifecycle, 1);
        }
        let Some(next) = self.next.checked_add(1) else {
            return fail(FailureCode::StorageExhausted, 2);
        };
        let request = RequestId(self.next);
        self.next = next;
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.empty, 0)
                .expect("pre-admitted empty keyboard request"),
        }
    }

    pub(crate) fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        if self.terminal || self.pending != Some(request) {
            return fail(FailureCode::InvalidLifecycle, 3);
        }
        self.pending = None;
        if let Some(failure) = outcome.failure {
            self.terminal = true;
            return OperationAction::Fail(failure);
        }
        match outcome.disposition {
            HostOperationDisposition::Completed => {
                let (Some(output), Some(canonical)) = (outcome.output, canonical) else {
                    return fail(FailureCode::InvalidInput, 4);
                };
                if self.validator.validate(canonical).is_err() {
                    return fail(FailureCode::InvalidInput, 5);
                }
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            HostOperationDisposition::Cancelled if outcome.output.is_none() => {
                self.terminal = true;
                fail(FailureCode::Cancelled, 0)
            }
            _ => fail(FailureCode::InvalidLifecycle, 6),
        }
    }
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::button::offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || !placement.authority.is_empty()
        || placement
            .resources
            .iter()
            .filter(|r| r.class_id.as_str() == conduit_core::INPUT_RESOURCE_CLASS)
            .count()
            != 1
        || placement
            .resources
            .iter()
            .any(|r| r.protected.is_some() || r.compute.is_some())
        || !placement.resources.iter().any(|r| {
            r.class_id.as_str() == conduit_core::INPUT_RESOURCE_CLASS
                && r.units == 1
                && r.protected.is_none()
                && r.compute.is_none()
        })
    {
        return Err("planned Space-button realization mismatch".into());
    }
    if !placement.configuration.is_empty() {
        return Err("standing Space-button has unexpected configuration".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let bytes = conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: bytes,
        host_requests: 1,
        sign_items: 64,
        maximum_value_bytes: bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let empty = values.store(&[]).map_err(|error| format!("{error:?}"))?;
    let validator = PreparedStructuredValueValidator::new(
        &conduit_semantic_catalog::input_button_transition_type(),
        conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES as usize,
    )
    .map_err(|error| format!("prepare button transition validator: {error:?}"))?;
    Ok(InstalledOperation::ButtonInput(ButtonOperation {
        empty,
        empty_released: false,
        next: 0,
        pending: None,
        terminal: false,
        validator,
    }))
}
