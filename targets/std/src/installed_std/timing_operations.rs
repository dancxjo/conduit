use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use super::timing_configuration::{self, TimingConfiguration};
use conduit_core::{encode_monotonic_duration, InfoBool, PlannedGear, PortDirection, BOOL_INFO_ID};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, OperationAction,
    OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static TIME_DEBOUNCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TIME_DEBOUNCE_IMPLEMENTATION,
    budget: debounce_budget,
    prepare: prepare_debounce,
};

pub(super) static TIME_TIMEOUT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TIME_TIMEOUT_IMPLEMENTATION,
    budget: timeout_budget,
    prepare: prepare_timeout,
};

pub(super) struct DebounceOperation {
    durations: Vec<ValueRef>,
    next_request: usize,
    maximum_values: usize,
    accepted_values: usize,
    pending: Option<RequestId>,
    cancellation: Option<RequestId>,
    candidate: Option<ValueRef>,
    released: Option<ValueRef>,
    terminal_releases: Vec<ValueRef>,
    retain_resumed: bool,
    closing: bool,
    complete_after_emit: bool,
}

pub(super) struct TimeoutOperation {
    durations: Vec<ValueRef>,
    false_values: Vec<ValueRef>,
    true_values: Vec<ValueRef>,
    next_request: usize,
    next_false: usize,
    next_true: usize,
    maximum_values: usize,
    accepted_values: usize,
    pending: Option<RequestId>,
    cancellation: Option<RequestId>,
    terminal_releases: Vec<ValueRef>,
    timed_out: bool,
    closing: bool,
    arm_after_emit: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for DebounceOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return outcome
                    .failure
                    .map_or_else(|| step_fail(780), StepOutcome::Fail);
            }
            match outcome.disposition {
                HostCallDisposition::Cancelled => {
                    if self.closing && self.candidate.is_some() && !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed debounce deadline cancellation");
                    self.pending = None;
                    self.cancellation = None;
                    if self.closing {
                        if let Some(value) = self.candidate.take() {
                            io.send(PortId(0), value)
                                .expect("ready final debounce output");
                            self.complete_after_emit = true;
                        }
                        return self.finish_cleanup(io);
                    }
                    if let Err(outcome) = self.request_deadline_step(io) {
                        return outcome;
                    }
                    return StepOutcome::Progress;
                }
                HostCallDisposition::Completed => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(value) = self.candidate.take() else {
                        return step_fail(779);
                    };
                    io.consume_host_completion()
                        .expect("observed debounce deadline completion");
                    io.send(PortId(0), value).expect("ready debounced output");
                    self.pending = None;
                    if self.closing {
                        self.complete_after_emit = true;
                        return self.finish_cleanup(io);
                    }
                    return StepOutcome::Progress;
                }
                _ => return step_fail(780),
            }
        }

        if let Some(value) = io.input(PortId(0)) {
            if self.closing || self.accepted_values >= self.maximum_values {
                return step_fail(780);
            }
            let retained = io.take_input(PortId(0)).expect("present debounce input");
            debug_assert_eq!(retained, value);
            if let Some(previous) = self.candidate.replace(retained) {
                io.discard(previous).expect("superseded debounce candidate");
            }
            self.accepted_values += 1;
            self.retain_resumed = false;
            if let Some(request) = self.pending {
                io.cancel_host_call(request)
                    .expect("debounce deadline cancellation");
                self.cancellation = Some(request);
            } else if let Err(outcome) = self.request_deadline_step(io) {
                return outcome;
            }
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) && !self.closing {
            io.consume_closed(PortId(0))
                .expect("observed debounce input closure");
            self.closing = true;
            self.release_unused_durations();
            if let Some(request) = self.pending {
                io.cancel_host_call(request)
                    .expect("closing debounce deadline cancellation");
                self.cancellation = Some(request);
                self.stage_terminal_releases(io);
                return StepOutcome::Progress;
            }
            if self.candidate.is_some() && !io.output_ready(PortId(0)) {
                return StepOutcome::Progress;
            }
            if let Some(value) = self.candidate.take() {
                io.send(PortId(0), value)
                    .expect("ready final debounce output");
                self.complete_after_emit = true;
            }
            return self.finish_cleanup(io);
        }

        if self.closing && self.pending.is_none() {
            if self.candidate.is_some() && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if let Some(value) = self.candidate.take() {
                io.send(PortId(0), value)
                    .expect("ready final debounce output");
                self.complete_after_emit = true;
            }
            return self.finish_cleanup(io);
        }
        StepOutcome::Await
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.cancellation = None;
        self.candidate = None;
        self.released = None;
    }
}

impl DebounceOperation {
    fn request_deadline_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
    ) -> Result<(), StepOutcome> {
        let Some(input) = self.durations.get(self.next_request).copied() else {
            return Err(step_fail(781));
        };
        let Ok(raw_request) = u32::try_from(self.next_request + 1) else {
            return Err(step_fail(782));
        };
        let request = RequestId(raw_request);
        let input =
            BoundedValueRef::new(input, 8).expect("deadline duration is exactly eight bytes");
        io.request_host_call(request, HostCallId(0), input)
            .expect("debounce deadline Host Call");
        self.next_request += 1;
        self.pending = Some(request);
        Ok(())
    }

    fn stage_terminal_releases<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        for _ in 0..PORTS {
            let Some(value) = self.terminal_releases.pop() else {
                break;
            };
            io.discard(value).expect("unused debounce duration");
        }
    }

    fn finish_cleanup<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        self.stage_terminal_releases(io);
        if self.terminal_releases.is_empty() {
            self.complete_after_emit = false;
            StepOutcome::Complete
        } else {
            StepOutcome::Progress
        }
    }
}

impl<const PORTS: usize> StepOperation<PORTS> for TimeoutOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return outcome
                    .failure
                    .map_or_else(|| step_fail(784), StepOutcome::Fail);
            }
            match outcome.disposition {
                HostCallDisposition::Cancelled => {
                    io.consume_host_completion()
                        .expect("observed timeout deadline cancellation");
                    self.pending = None;
                    self.cancellation = None;
                    if self.closing {
                        return self.finish_cleanup(io);
                    }
                    if let Err(outcome) = self.request_deadline_step(io) {
                        return outcome;
                    }
                    return StepOutcome::Progress;
                }
                HostCallDisposition::Completed => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(value) = self.true_values.get(self.next_true).copied() else {
                        return step_fail(788);
                    };
                    io.consume_host_completion()
                        .expect("observed timeout deadline completion");
                    io.send(PortId(0), value).expect("ready timeout output");
                    self.pending = None;
                    self.next_true += 1;
                    self.timed_out = true;
                    return StepOutcome::Progress;
                }
                _ => return step_fail(784),
            }
        }

        if io.input(PortId(0)).is_some() {
            if self.closing || self.accepted_values >= self.maximum_values {
                return step_fail(784);
            }
            if self.timed_out && self.pending.is_none() && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).expect("present timeout input");
            self.accepted_values += 1;
            if let Some(request) = self.pending {
                io.cancel_host_call(request)
                    .expect("timeout deadline cancellation");
                self.cancellation = Some(request);
                return StepOutcome::Progress;
            }
            if self.timed_out {
                self.timed_out = false;
                if let Err(outcome) = self.emit_false_and_arm_step(io) {
                    return outcome;
                }
                return StepOutcome::Progress;
            }
            return step_fail(783);
        }

        if io.input_closed(PortId(0)) && !self.closing {
            io.consume_closed(PortId(0))
                .expect("observed timeout input closure");
            self.closing = true;
            self.release_unused_values();
            if let Some(request) = self.pending {
                io.cancel_host_call(request)
                    .expect("closing timeout deadline cancellation");
                self.cancellation = Some(request);
                self.stage_terminal_releases(io);
                return StepOutcome::Progress;
            }
            return self.finish_cleanup(io);
        }

        if self.closing && self.pending.is_none() {
            return self.finish_cleanup(io);
        }
        if self.next_request == 0 && self.next_false == 0 && self.accepted_values == 0 {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if let Err(outcome) = self.emit_false_and_arm_step(io) {
                return outcome;
            }
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.cancellation = None;
    }
}

impl TimeoutOperation {
    fn emit_false_and_arm_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
    ) -> Result<(), StepOutcome> {
        let Some(value) = self.false_values.get(self.next_false).copied() else {
            return Err(step_fail(785));
        };
        io.send(PortId(0), value).expect("ready non-timeout output");
        self.next_false += 1;
        self.arm_after_emit = false;
        self.request_deadline_step(io)
    }

    fn request_deadline_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
    ) -> Result<(), StepOutcome> {
        let Some(input) = self.durations.get(self.next_request).copied() else {
            return Err(step_fail(786));
        };
        let Ok(raw_request) = u32::try_from(self.next_request + 1) else {
            return Err(step_fail(787));
        };
        let request = RequestId(raw_request);
        let input =
            BoundedValueRef::new(input, 8).expect("deadline duration is exactly eight bytes");
        io.request_host_call(request, HostCallId(0), input)
            .expect("timeout deadline Host Call");
        self.next_request += 1;
        self.pending = Some(request);
        Ok(())
    }

    fn stage_terminal_releases<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        for _ in 0..PORTS {
            let Some(value) = self.terminal_releases.pop() else {
                break;
            };
            io.discard(value).expect("unused timeout value");
        }
    }

    fn finish_cleanup<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        self.stage_terminal_releases(io);
        if self.terminal_releases.is_empty() {
            StepOutcome::Complete
        } else {
            StepOutcome::Progress
        }
    }
}

const fn step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl DebounceOperation {
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
                self.released = self.candidate.replace(value);
                if let Some(request) = self.pending {
                    self.cancellation = Some(request);
                    OperationAction::Await
                } else {
                    self.request_deadline()
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Cancelled
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                if self.closing {
                    self.flush_and_complete()
                } else {
                    self.request_deadline()
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.candidate.take().map_or_else(
                    || fail(779),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            OperationInput::Closed { port: PortId(0) } if !self.closing => {
                self.closing = true;
                self.release_unused_durations();
                if let Some(request) = self.pending {
                    self.cancellation = Some(request);
                    OperationAction::Await
                } else {
                    self.flush_and_complete()
                }
            }
            _ => fail(780),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.complete_after_emit {
            self.complete_after_emit = false;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released
            .take()
            .or_else(|| self.terminal_releases.pop())
    }

    pub(super) fn take_host_call_cancellation(&mut self) -> Option<RequestId> {
        self.cancellation.take()
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.durations.capacity() + self.terminal_releases.capacity()
    }

    fn request_deadline(&mut self) -> OperationAction {
        let Some(input) = self.durations.get(self.next_request).copied() else {
            return fail(781);
        };
        let Ok(raw_request) = u32::try_from(self.next_request + 1) else {
            return fail(782);
        };
        self.next_request += 1;
        let request = RequestId(raw_request);
        self.pending = Some(request);
        OperationAction::RequestHostCall {
            request,
            operation: HostCallId(0),
            input: BoundedValueRef::new(input, 8)
                .expect("deadline duration is exactly eight bytes"),
        }
    }

    fn flush_and_complete(&mut self) -> OperationAction {
        self.candidate
            .take()
            .map_or(OperationAction::Complete, |value| {
                self.complete_after_emit = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            })
    }

    fn release_unused_durations(&mut self) {
        self.terminal_releases
            .extend(self.durations.drain(self.next_request..));
    }
}

impl TimeoutOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.emit_false_and_arm()
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0), ..
            } if !self.closing && self.accepted_values < self.maximum_values => {
                self.accepted_values += 1;
                if let Some(request) = self.pending {
                    self.cancellation = Some(request);
                    OperationAction::Await
                } else if self.timed_out {
                    self.timed_out = false;
                    self.emit_false_and_arm()
                } else {
                    fail(783)
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Cancelled
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                if self.closing {
                    OperationAction::Complete
                } else {
                    self.request_deadline()
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostCallDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.timed_out = true;
                self.next_true_value()
                    .map_or_else(fail_timeout_value, |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    })
            }
            OperationInput::Closed { port: PortId(0) } if !self.closing => {
                self.closing = true;
                self.release_unused_values();
                if let Some(request) = self.pending {
                    self.cancellation = Some(request);
                    OperationAction::Await
                } else {
                    OperationAction::Complete
                }
            }
            _ => fail(784),
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

    pub(super) fn take_host_call_cancellation(&mut self) -> Option<RequestId> {
        self.cancellation.take()
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.terminal_releases.pop()
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.durations.capacity()
            + self.false_values.capacity()
            + self.true_values.capacity()
            + self.terminal_releases.capacity()
    }

    fn emit_false_and_arm(&mut self) -> OperationAction {
        let Some(value) = self.false_values.get(self.next_false).copied() else {
            return fail(785);
        };
        self.next_false += 1;
        self.arm_after_emit = true;
        OperationAction::Emit {
            port: PortId(0),
            value,
        }
    }

    fn next_true_value(&mut self) -> Option<ValueRef> {
        let value = self.true_values.get(self.next_true).copied()?;
        self.next_true += 1;
        Some(value)
    }

    fn request_deadline(&mut self) -> OperationAction {
        let Some(input) = self.durations.get(self.next_request).copied() else {
            return fail(786);
        };
        let Ok(raw_request) = u32::try_from(self.next_request + 1) else {
            return fail(787);
        };
        self.next_request += 1;
        let request = RequestId(raw_request);
        self.pending = Some(request);
        OperationAction::RequestHostCall {
            request,
            operation: HostCallId(0),
            input: BoundedValueRef::new(input, 8)
                .expect("deadline duration is exactly eight bytes"),
        }
    }

    fn release_unused_values(&mut self) {
        self.terminal_releases
            .extend(self.durations.drain(self.next_request..));
        self.terminal_releases
            .extend(self.false_values.drain(self.next_false..));
        self.terminal_releases
            .extend(self.true_values.drain(self.next_true..));
    }
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

fn fail_timeout_value() -> OperationAction {
    fail(788)
}

fn debounce_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_debounce(placement)?;
    let configuration = timing_configuration::parse(placement, true)?;
    timing_configuration::budget(
        configuration.maximum_values,
        configuration.maximum_values,
        0,
    )
}

fn timeout_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_timeout(placement)?;
    let configuration = timing_configuration::parse(placement, false)?;
    let requests = configuration.maximum_values + 1;
    timing_configuration::budget(requests, requests, requests + configuration.maximum_values)
}

fn prepare_debounce(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_debounce(placement)?;
    let configuration = timing_configuration::parse(placement, true)?;
    Ok(InstalledOperation::TimeDebounce(DebounceOperation {
        durations: store_durations(values, configuration)?,
        next_request: 0,
        maximum_values: configuration.maximum_values,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        candidate: None,
        released: None,
        terminal_releases: Vec::with_capacity(
            conduit_semantic_catalog::TIME_MAXIMUM_VALUES as usize,
        ),
        retain_resumed: false,
        closing: false,
        complete_after_emit: false,
    }))
}

fn prepare_timeout(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_timeout(placement)?;
    let mut configuration = timing_configuration::parse(placement, false)?;
    let maximum_values = configuration.maximum_values;
    configuration.maximum_values += 1;
    let false_values = store_bool_values(values, configuration.maximum_values, InfoBool::FALSE)?;
    let true_values = store_bool_values(values, maximum_values, InfoBool::TRUE)?;
    Ok(InstalledOperation::TimeTimeout(TimeoutOperation {
        durations: store_durations(values, configuration)?,
        false_values,
        true_values,
        next_request: 0,
        next_false: 0,
        next_true: 0,
        maximum_values,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        terminal_releases: Vec::with_capacity(
            conduit_semantic_catalog::TIME_TIMEOUT_MAXIMUM_VALUES as usize * 3,
        ),
        timed_out: false,
        closing: false,
        arm_after_emit: false,
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
                .map_err(|error| format!("store admitted deadline duration: {error:?}"))
        })
        .collect()
}

fn store_bool_values(
    values: &mut conduit_kernel::HostedValueStore,
    count: usize,
    value: InfoBool,
) -> Result<Vec<ValueRef>, String> {
    (0..count)
        .map(|_| {
            values
                .store(&value.encode())
                .map_err(|error| format!("store timeout state: {error:?}"))
        })
        .collect()
}

fn validate_debounce(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        &conduit_std_offers::time_debounce_offer(),
        conduit_semantic_catalog::TIME_DEBOUNCE_KIND,
    )
}

fn validate_timeout(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        &conduit_std_offers::time_timeout_offer(),
        conduit_semantic_catalog::TIME_TIMEOUT_KIND,
    )
}

fn validate(
    placement: &PlannedGear,
    offer: &conduit_core::CapabilityOffer,
    kind: &str,
) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.capability_id != offer.capability_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.limits != offer.limits
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || placement
            .inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
        || placement.outputs.iter().any(|port| {
            port.direction != PortDirection::Output || port.value_kind.as_str() != BOOL_INFO_ID
        })
    {
        return Err(format!(
            "planned {kind} executable identity does not match its installation"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "timing_operations_tests.rs"]
mod tests;
