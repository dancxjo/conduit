//! Installed tick/every sources and the tick-observer test fixture.

use super::contract::{
    encode_tick, parse_every_configuration, parse_tick_configuration, TickConfiguration,
    TICK_ARTIFACT, TICK_CONTRACT_REVISION, TICK_ENCODED_LEN, TICK_EXECUTION_PROFILE,
    TICK_IMPLEMENTATION, TICK_KIND, TICK_VALUE_KIND, TIME_EVERY_ARTIFACT,
    TIME_EVERY_CONTRACT_REVISION, TIME_EVERY_EXECUTION_PROFILE, TIME_EVERY_IMPLEMENTATION,
    TIME_EVERY_KIND,
};
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    RequestId, ValueRef, ValueStorage,
};

pub(super) static TICK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TICK_IMPLEMENTATION,
    budget: tick_budget,
    prepare: prepare_tick,
};

pub(super) static EVERY_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TIME_EVERY_IMPLEMENTATION,
    budget: every_budget,
    prepare: prepare_every,
};

pub(super) struct TickOperation {
    values: Vec<ValueRef>,
    waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl TickOperation {
    pub(super) fn allocation_capacity(&self) -> usize {
        self.values.capacity() + self.waits.capacity()
    }

    pub(super) fn start(&mut self) -> OperationAction {
        self.request_wait().unwrap_or(OperationAction::Complete)
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.values.get(self.next).copied().map_or_else(
                    || InstalledOperation::fail(1),
                    |value| OperationAction::Emit {
                        port: conduit_kernel::PortId(0),
                        value,
                    },
                )
            }
            _ => InstalledOperation::fail(2),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next += 1;
        self.request_wait().unwrap_or(OperationAction::Complete)
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    fn request_wait(&mut self) -> Option<OperationAction> {
        let wait = self.waits.get(self.next).copied()?;
        let request = RequestId(u32::try_from(self.next).ok()?);
        self.pending = Some(request);
        Some(OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(wait, TICK_ENCODED_LEN)
                .expect("wait duration is exactly eight bytes"),
        })
    }
}

fn tick_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_tick_placement(placement)?;
    let configuration =
        parse_tick_configuration(&placement.configuration).map_err(|error| error.to_string())?;
    tick_budget_for(&configuration)
}

fn every_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_every_placement(placement)?;
    tick_budget_for(&parse_every_configuration(&placement.configuration)?)
}

fn tick_budget_for(configuration: &TickConfiguration) -> Result<OperationBudget, String> {
    let value_items = configuration
        .count
        .checked_mul(2)
        .and_then(|count| u16::try_from(count.max(1)).ok())
        .ok_or_else(|| "tick value item budget overflow".to_string())?;
    let value_bytes = configuration
        .count
        .checked_mul(u64::from(TICK_ENCODED_LEN) * 2)
        .and_then(|bytes| u32::try_from(bytes.max(1)).ok())
        .ok_or_else(|| "tick value byte budget overflow".to_string())?;
    let sign_items = configuration
        .count
        .checked_mul(15)
        .and_then(|items| items.checked_add(64))
        .and_then(|items| u16::try_from(items).ok())
        .ok_or_else(|| "tick sign item budget overflow".to_string())?;
    Ok(OperationBudget {
        value_items,
        value_bytes,
        host_requests: usize::try_from(configuration.count)
            .map_err(|_| "tick request budget overflow".to_string())?,
        sign_items,
        maximum_value_bytes: TICK_ENCODED_LEN,
    })
}

fn prepare_tick(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_tick_placement(placement)?;
    let configuration =
        parse_tick_configuration(&placement.configuration).map_err(|error| error.to_string())?;
    prepare_tick_values(configuration, values)
}

fn prepare_every(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_every_placement(placement)?;
    prepare_tick_values(parse_every_configuration(&placement.configuration)?, values)
}

fn prepare_tick_values(
    configuration: TickConfiguration,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let count = usize::try_from(configuration.count)
        .map_err(|_| "tick count does not fit hosted preparation".to_string())?;
    let mut ticks = Vec::with_capacity(count);
    let mut waits = Vec::with_capacity(count);
    for sequence in 0..configuration.count {
        ticks.push(
            values
                .store(&encode_tick(sequence))
                .map_err(|error| format!("store typed tick: {error:?}"))?,
        );
        waits.push(
            values
                .store(&configuration.period_ms.to_le_bytes())
                .map_err(|error| format!("store tick wait: {error:?}"))?,
        );
    }
    Ok(InstalledOperation::Tick(TickOperation {
        values: ticks,
        waits,
        next: 0,
        pending: None,
    }))
}

fn validate_tick_placement(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != TICK_KIND
        || placement.kind_contract_revision.as_str() != TICK_CONTRACT_REVISION
        || placement.execution_profile_id.as_str() != TICK_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != TICK_IMPLEMENTATION
        || placement.artifact_id.as_str() != TICK_ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs.len() != 1
        || placement.outputs[0].port_id.as_str() != "tick"
        || placement.outputs[0].value_kind.as_str() != TICK_VALUE_KIND
        || placement.outputs[0].direction != PortDirection::Output
    {
        return Err("planned tick executable identity does not match its installation".to_string());
    }
    Ok(())
}

fn validate_every_placement(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != TIME_EVERY_KIND
        || placement.kind_contract_revision.as_str() != TIME_EVERY_CONTRACT_REVISION
        || placement.execution_profile_id.as_str() != TIME_EVERY_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != TIME_EVERY_IMPLEMENTATION
        || placement.artifact_id.as_str() != TIME_EVERY_ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs.len() != 1
        || placement.outputs[0].port_id.as_str() != "tick"
        || placement.outputs[0].value_kind.as_str() != TICK_VALUE_KIND
        || placement.outputs[0].direction != PortDirection::Output
    {
        return Err("planned time/every identity does not match its installation".to_string());
    }
    Ok(())
}

#[cfg(test)]
pub(super) const TEST_OBSERVER_KIND: &str = "conduit.test/tick-observer";
#[cfg(test)]
pub(super) const TEST_OBSERVER_IMPLEMENTATION: &str = "conduit.test/tick-observer@1";

#[cfg(test)]
pub(super) static TEST_OBSERVER_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TEST_OBSERVER_IMPLEMENTATION,
    budget: |_| {
        Ok(OperationBudget {
            value_items: 0,
            value_bytes: 0,
            host_requests: 0,
            sign_items: 16,
            maximum_value_bytes: TICK_ENCODED_LEN,
        })
    },
    prepare: prepare_test_observer,
};

#[cfg(test)]
pub(super) struct TestObserverOperation {
    pending: Option<RequestId>,
    next: u32,
}

#[cfg(test)]
impl TestObserverOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: conduit_kernel::PortId(0),
                value,
            } if self.pending.is_none() => {
                let request = RequestId(0x8000_0000 | self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, TICK_ENCODED_LEN)
                        .expect("typed tick is exactly eight bytes"),
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
            OperationInput::Closed {
                port: conduit_kernel::PortId(0),
            } if self.pending.is_none() => OperationAction::Complete,
            _ => InstalledOperation::fail(3),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

#[cfg(test)]
fn prepare_test_observer(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    if placement.kind_id.as_str() != TEST_OBSERVER_KIND
        || placement.implementation_id.as_str() != TEST_OBSERVER_IMPLEMENTATION
        || placement.inputs.len() != 1
        || !placement.outputs.is_empty()
        || placement.inputs[0].port_id.as_str() != "in"
        || placement.inputs[0].value_kind.as_str() != TICK_VALUE_KIND
        || placement.inputs[0].direction != PortDirection::Input
    {
        return Err("test observer executable identity does not match its fixture".to_string());
    }
    Ok(InstalledOperation::TestObserver(TestObserverOperation {
        pending: None,
        next: 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{Failure, FailureCode, HostOperationOutcome};

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: TICK_ENCODED_LEN,
        }
    }

    #[test]
    fn cancelled_tick_rejects_a_late_timer_completion() {
        let mut operation = TickOperation {
            values: vec![value(0)],
            waits: vec![value(1)],
            next: 0,
            pending: None,
        };
        assert!(matches!(
            operation.start(),
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                operation: HostOperationId(0),
                ..
            }
        ));
        operation.cancel();
        assert!(matches!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 2,
            })
        ));
    }
}
