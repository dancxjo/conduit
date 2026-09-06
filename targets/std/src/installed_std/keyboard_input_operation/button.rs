//! Finite Space-key to semantic button implementation using the existing input operation.
pub(in crate::installed_std) mod indicator;
#[cfg(test)]
mod tests;
use super::super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_human::{KeyEvent, KeyTransition};
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
    transitions: Vec<ValueRef>,
    empty_released: bool,
    emitted: usize,
    next: u32,
    pending: Option<RequestId>,
    held: bool,
    terminal: bool,
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
        self.transitions.capacity()
    }

    fn request(&mut self) -> OperationAction {
        if self.terminal || self.pending.is_some() {
            return fail(FailureCode::InvalidLifecycle, 1);
        }
        if self.emitted == self.transitions.len() {
            self.terminal = true;
            return OperationAction::Complete;
        }
        if self.next == super::MAX_PLAY_EVENTS {
            return fail(FailureCode::StorageExhausted, 2);
        }
        let request = RequestId(self.next);
        self.next += 1;
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
                let Some(event) = canonical.and_then(|bytes| KeyEvent::decode(bytes).ok()) else {
                    return fail(FailureCode::InvalidInput, 4);
                };
                if event.usage() != 0x2c {
                    return self.request();
                }
                let pressed = event.transition() == KeyTransition::Pressed;
                if pressed == self.held {
                    return fail(FailureCode::InvalidInput, 5);
                }
                self.held = pressed;
                let value = self.transitions[self.emitted];
                self.emitted += 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
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

fn validate(placement: &PlannedGear) -> Result<usize, String> {
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
        || placement.resources.len() != 1
        || !placement.resources.iter().all(|r| {
            r.class_id.as_str() == conduit_core::INPUT_RESOURCE_CLASS
                && r.units == 1
                && r.protected.is_none()
                && r.compute.is_none()
        })
    {
        return Err("planned Space-button realization mismatch".into());
    }
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-transitions", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("Space-button lacks maximum-transitions")?;
    if !(1..=u64::from(conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_VALUES))
        .contains(&maximum)
    {
        return Err("Space-button maximum outside admitted bounds".into());
    }
    Ok(maximum as usize)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let maximum = validate(placement)?;
    let bytes = conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES;
    Ok(OperationBudget {
        value_items: (maximum * 2 + 2) as u16,
        value_bytes: bytes * maximum as u32 * 2 + conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        host_requests: super::MAX_PLAY_EVENTS as usize,
        sign_items: (super::MAX_PLAY_EVENTS * 6) as u16,
        maximum_value_bytes: bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let maximum = validate(placement)?;
    let mut transitions = Vec::with_capacity(maximum);
    for sequence in 0..maximum {
        // Initial released state plus duplicate/unmatched rejection makes the
        // accepted sequence alternate exactly. No unused alternative is stored.
        let bytes = conduit_semantic_catalog::button_transition_value(
            "button/primary",
            sequence % 2 == 0,
            sequence as u64,
        )
        .and_then(|value| value.canonical_bytes())
        .map_err(|error| format!("{error:?}"))?;
        transitions.push(values.store(&bytes).map_err(|error| format!("{error:?}"))?);
    }
    let empty = values.store(&[]).map_err(|error| format!("{error:?}"))?;
    Ok(InstalledOperation::ButtonInput(ButtonOperation {
        empty,
        empty_released: false,
        transitions,
        emitted: 0,
        next: 0,
        pending: None,
        held: false,
        terminal: false,
    }))
}
