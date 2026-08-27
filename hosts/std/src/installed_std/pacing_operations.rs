use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use super::timing_configuration::{self, TimingConfiguration};
use conduit_core::{encode_monotonic_duration, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static TIME_DELAY_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TIME_DELAY_IMPLEMENTATION,
    budget: delay_budget,
    prepare: prepare_delay,
};

pub(super) static TIME_THROTTLE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TIME_THROTTLE_IMPLEMENTATION,
    budget: throttle_budget,
    prepare: prepare_throttle,
};

pub(super) struct DelayOperation {
    durations: Vec<ValueRef>,
    values: Vec<ValueRef>,
    terminal_releases: Vec<ValueRef>,
    next_request: usize,
    next_value: usize,
    maximum_values: usize,
    pending: Option<RequestId>,
    accepted_values: usize,
    retain_resumed: bool,
    closing: bool,
    continue_after_emit: bool,
}

pub(super) struct ThrottleOperation {
    durations: Vec<ValueRef>,
    terminal_releases: Vec<ValueRef>,
    next_request: usize,
    maximum_values: usize,
    accepted_values: usize,
    pending: Option<RequestId>,
    cancellation: Option<RequestId>,
    arm_after_emit: bool,
    closing: bool,
}

impl DelayOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.closing && self.accepted_values < self.maximum_values => {
                self.accepted_values += 1;
                self.retain_resumed = true;
                self.values.push(value);
                if self.pending.is_none() && self.next_value + 1 == self.values.len() {
                    self.request_deadline()
                } else {
                    OperationAction::Await
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                let Some(value) = self.values.get(self.next_value).copied() else {
                    return fail(886);
                };
                self.next_value += 1;
                self.continue_after_emit = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.closing => {
                self.closing = true;
                if self.pending.is_some() {
                    OperationAction::Await
                } else if self.next_value < self.values.len() {
                    self.request_deadline()
                } else {
                    self.release_unused_durations();
                    OperationAction::Complete
                }
            }
            _ => fail(887),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if !self.continue_after_emit {
            return OperationAction::Await;
        }
        self.continue_after_emit = false;
        if self.next_value < self.values.len() {
            self.request_deadline()
        } else if self.closing {
            self.release_unused_durations();
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.values.clear();
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.terminal_releases.pop()
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.durations.capacity() + self.values.capacity() + self.terminal_releases.capacity()
    }

    fn request_deadline(&mut self) -> OperationAction {
        let Some(input) = self.durations.get(self.next_request).copied() else {
            return fail(888);
        };
        let Ok(raw_request) = u32::try_from(self.next_request + 1) else {
            return fail(889);
        };
        self.next_request += 1;
        let request = RequestId(raw_request);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(input, 8).expect("delay duration is exactly eight bytes"),
        }
    }

    fn release_unused_durations(&mut self) {
        self.terminal_releases
            .extend(self.durations.drain(self.next_request..));
    }
}

impl ThrottleOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.closing && self.accepted_values < self.maximum_values => {
                self.accepted_values += 1;
                if self.pending.is_some() || self.arm_after_emit {
                    OperationAction::Await
                } else {
                    self.arm_after_emit = true;
                    OperationAction::Emit {
                        port: PortId(0),
                        value,
                    }
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                OperationAction::Await
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Cancelled
                    && outcome.output.is_none()
                    && outcome.failure.is_none()
                    && self.closing =>
            {
                self.pending = None;
                self.release_unused_durations();
                OperationAction::Complete
            }
            OperationInput::Closed { port: PortId(0) } if !self.closing => {
                self.closing = true;
                if let Some(request) = self.pending {
                    self.cancellation = Some(request);
                    OperationAction::Await
                } else {
                    self.release_unused_durations();
                    OperationAction::Complete
                }
            }
            _ => fail(890),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.arm_after_emit {
            self.arm_after_emit = false;
            self.request_deadline()
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.cancellation = None;
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.terminal_releases.pop()
    }

    pub(super) fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        self.cancellation.take()
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.durations.capacity() + self.terminal_releases.capacity()
    }

    fn request_deadline(&mut self) -> OperationAction {
        let Some(input) = self.durations.get(self.next_request).copied() else {
            return fail(891);
        };
        let Ok(raw_request) = u32::try_from(self.next_request + 1) else {
            return fail(892);
        };
        self.next_request += 1;
        let request = RequestId(raw_request);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(input, 8)
                .expect("throttle duration is exactly eight bytes"),
        }
    }

    fn release_unused_durations(&mut self) {
        self.terminal_releases
            .extend(self.durations.drain(self.next_request..));
    }
}

fn delay_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_offers::time_delay_offer())?;
    let configuration = timing_configuration::parse_pacing(placement, None)?;
    timing_configuration::budget(
        configuration.maximum_values,
        configuration.maximum_values,
        configuration.maximum_values,
    )
}

fn throttle_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_offers::time_throttle_offer())?;
    let configuration = timing_configuration::parse_pacing(
        placement,
        Some(conduit_std_catalog::TIME_POLICY_LEADING),
    )?;
    timing_configuration::budget(
        configuration.maximum_values,
        configuration.maximum_values,
        0,
    )
}

fn prepare_delay(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, &conduit_std_offers::time_delay_offer())?;
    let configuration = timing_configuration::parse_pacing(placement, None)?;
    let maximum_values = configuration.maximum_values;
    Ok(InstalledOperation::TimeDelay(DelayOperation {
        durations: store_durations(values, configuration)?,
        values: Vec::with_capacity(maximum_values),
        terminal_releases: Vec::with_capacity(maximum_values),
        next_request: 0,
        next_value: 0,
        maximum_values,
        pending: None,
        accepted_values: 0,
        retain_resumed: false,
        closing: false,
        continue_after_emit: false,
    }))
}

fn prepare_throttle(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, &conduit_std_offers::time_throttle_offer())?;
    let configuration = timing_configuration::parse_pacing(
        placement,
        Some(conduit_std_catalog::TIME_POLICY_LEADING),
    )?;
    let maximum_values = configuration.maximum_values;
    Ok(InstalledOperation::TimeThrottle(ThrottleOperation {
        durations: store_durations(values, configuration)?,
        terminal_releases: Vec::with_capacity(maximum_values),
        next_request: 0,
        maximum_values,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        arm_after_emit: false,
        closing: false,
    }))
}

fn store_durations(
    values: &mut conduit_kernel::HostedValueStore,
    configuration: TimingConfiguration,
) -> Result<Vec<ValueRef>, String> {
    (0..configuration.maximum_values)
        .map(|_| {
            values
                .store(&encode_monotonic_duration(configuration.duration_ms))
                .map_err(|error| format!("store admitted pacing duration: {error:?}"))
        })
        .collect()
}

fn validate(placement: &PlannedGear, offer: &conduit_core::CapabilityOffer) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.capability_id != offer.capability_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.limits != offer.limits
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned pacing identity does not match its installation".to_string());
    }
    Ok(())
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

#[cfg(test)]
#[path = "pacing_operations_tests.rs"]
mod tests;
