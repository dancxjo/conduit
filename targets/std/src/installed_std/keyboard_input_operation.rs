//! Installed source state machine for a host-adapted portable keyboard.
pub(super) mod button;

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, FailureCode, HostOperationDisposition, HostOperationId, HostOperationOutcome,
    OperationAction, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) const MAX_PLAY_EVENTS: u32 = 64;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::HOSTED_KEYBOARD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct KeyboardInputOperation {
    empty_input: ValueRef,
    pending: Option<RequestId>,
    next_request: u32,
    emitted: bool,
}

impl KeyboardInputOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.request_next()
    }

    pub(super) fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        if self.pending != Some(request) || outcome.failure.is_some() {
            return outcome
                .failure
                .map_or_else(|| InstalledOperation::fail(110), OperationAction::Fail);
        }
        self.pending = None;
        match outcome.disposition {
            HostOperationDisposition::Completed => {
                let Some(canonical) = canonical else {
                    return InstalledOperation::fail(111);
                };
                if conduit_human::KeyEvent::decode(canonical).is_err() {
                    return fail(FailureCode::InvalidInput, 112);
                }
                let Ok(value) = conduit_kernel::CanonicalValue::new(canonical) else {
                    return fail(FailureCode::StorageExhausted, 113);
                };
                self.emitted = true;
                OperationAction::EmitCanonical {
                    port: PortId(0),
                    value,
                }
            }
            HostOperationDisposition::Cancelled if outcome.output.is_none() => {
                OperationAction::Complete
            }
            HostOperationDisposition::Denied
            | HostOperationDisposition::Failed
            | HostOperationDisposition::Cancelled => InstalledOperation::fail(114),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if !self.emitted {
            return InstalledOperation::fail(115);
        }
        self.emitted = false;
        self.request_next()
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.emitted = false;
    }

    fn request_next(&mut self) -> OperationAction {
        if self.pending.is_some() || self.emitted {
            return InstalledOperation::fail(116);
        }
        if self.next_request == MAX_PLAY_EVENTS {
            return fail(FailureCode::StorageExhausted, 117);
        }
        let request = RequestId(self.next_request);
        self.next_request += 1;
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.empty_input, 0)
                .expect("keyboard request input is exactly empty"),
        }
    }
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure { code, detail })
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        host_requests: MAX_PLAY_EVENTS as usize,
        sign_items: (MAX_PLAY_EVENTS as u16).saturating_mul(4),
        maximum_value_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let empty_input = values
        .store(&[])
        .map_err(|error| format!("reserve empty keyboard request: {error:?}"))?;
    Ok(InstalledOperation::KeyboardInput(KeyboardInputOperation {
        empty_input,
        pending: None,
        next_request: 0,
        emitted: false,
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let contract = conduit_semantic_catalog::keyboard_contract();
    let operation = conduit_std_offers::next_key_event_host_operation_requirement();
    if placement.kind_id != contract.kind_id
        || placement.kind_contract_revision
            != conduit_semantic_catalog::keyboard_contract_revision()
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::HOSTED_KEYBOARD_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::HOSTED_KEYBOARD_IMPLEMENTATION
        || !placement.inputs.is_empty()
        || placement.outputs != contract.outputs
        || placement.outputs[0].direction != PortDirection::Output
        || placement.host_operations != [operation]
        || placement.limits != contract.limits
        || placement.resources.iter().all(|binding| {
            binding.class_id.as_str() != conduit_core::INPUT_RESOURCE_CLASS
                || binding.units != 1
                || binding.protected.is_some()
                || binding.compute.is_some()
        })
        || !placement.authority.is_empty()
    {
        return Err("planned hosted keyboard identity/resource contract mismatch".into());
    }
    Ok(())
}
