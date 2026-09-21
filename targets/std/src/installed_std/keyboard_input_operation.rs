//! Installed source state machine for a host-adapted portable keyboard.
pub(super) mod button;

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

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

impl<const PORTS: usize> StepBack<PORTS> for KeyboardInputOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.emitted {
            self.emitted = false;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) || outcome.failure.is_some() {
                return outcome.failure.map_or_else(
                    || step_fail(FailureCode::InvalidLifecycle, 110),
                    StepOutcome::Fail,
                );
            }
            match outcome.disposition {
                HostCallDisposition::Completed => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(canonical) = input_bytes.host_output() else {
                        return step_fail(FailureCode::InvalidLifecycle, 111);
                    };
                    if conduit_human::KeyEvent::decode(canonical).is_err() {
                        return step_fail(FailureCode::InvalidInput, 112);
                    }
                    let Ok(value) = conduit_kernel::CanonicalValue::new(canonical) else {
                        return step_fail(FailureCode::StorageExhausted, 113);
                    };
                    io.consume_host_completion()
                        .expect("observed keyboard completion");
                    io.send_canonical(PortId(0), value)
                        .expect("ready keyboard event output");
                    self.pending = None;
                    self.emitted = true;
                    return StepOutcome::Progress;
                }
                HostCallDisposition::Cancelled if outcome.output.is_none() => {
                    io.consume_host_completion()
                        .expect("observed keyboard cancellation");
                    self.pending = None;
                    return StepOutcome::Complete;
                }
                HostCallDisposition::Denied
                | HostCallDisposition::Failed
                | HostCallDisposition::Cancelled => {
                    return step_fail(FailureCode::InvalidLifecycle, 114)
                }
            }
        }

        if self.pending.is_none() {
            let request = RequestId(self.next_request);
            let Some(next_request) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::StorageExhausted, 117);
            };
            let input = BoundedValueRef::new(self.empty_input, 0)
                .expect("keyboard request input is exactly empty");
            io.request_host_call(request, HostCallId(0), input)
                .expect("keyboard Host Call");
            self.next_request = next_request;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.emitted = false;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl KeyboardInputOperation {}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        host_requests: 1,
        sign_items: 64,
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
    let operation = conduit_std_offers::next_key_event_host_call_requirement();
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
        || placement.host_calls != [operation]
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
