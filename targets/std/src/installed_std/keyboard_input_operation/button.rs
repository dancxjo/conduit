//! Standing Space-key to semantic button implementation.
pub(in crate::installed_std) mod indicator;
#[cfg(test)]
mod tests;
use super::super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PreparedStructuredValueValidator};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostedValueStore,
    PortId, RequestId, ValueRef, ValueStorage,
};

pub(crate) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::button::IMPLEMENTATION,
    budget,
    prepare,
};

pub(crate) struct ButtonOperation {
    empty: ValueRef,
    next: u32,
    pending: Option<RequestId>,
    terminal: bool,
    validator: PreparedStructuredValueValidator,
}

impl<const PORTS: usize> StepBack<PORTS> for ButtonOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return button_step_fail(FailureCode::InvalidLifecycle, 3);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            match outcome.disposition {
                HostCallDisposition::Completed => {
                    let (Some(output), Some(canonical)) =
                        (outcome.output, input_bytes.host_output())
                    else {
                        return button_step_fail(FailureCode::InvalidInput, 4);
                    };
                    if self.validator.validate(canonical).is_err() {
                        return button_step_fail(FailureCode::InvalidInput, 5);
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed button Host Call completion");
                    io.send(PortId(0), output.value)
                        .expect("ready button output");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                HostCallDisposition::Cancelled if outcome.output.is_none() => {
                    return button_step_fail(FailureCode::Cancelled, 0)
                }
                _ => return button_step_fail(FailureCode::InvalidLifecycle, 6),
            }
        }
        if self.pending.is_some() {
            return StepOutcome::Await;
        }
        let request = RequestId(self.next);
        let Some(next) = self.next.checked_add(1) else {
            return button_step_fail(FailureCode::StorageExhausted, 2);
        };
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(self.empty, 0).expect("pre-admitted empty keyboard request"),
        )
        .expect("button Host Call");
        self.next = next;
        self.pending = Some(request);
        StepOutcome::Progress
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.terminal = true;
    }
}

const fn button_step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl ButtonOperation {
    pub(crate) fn allocation_capacity(&self) -> usize {
        0
    }
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
        || placement.host_calls != offer.host_calls
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
        next: 0,
        pending: None,
        terminal: false,
        validator,
    }))
}
