//! Exact bounded processing-time window over one checked finite value specialization.

use super::back::{BackBudget, BackFactory, InstalledBack};
use super::timing_configuration;
use conduit_core::{
    encode_monotonic_duration, CheckedValueContract, FrontValueLocation, PlannedGear,
    PreparedLeafSequenceEncoder,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedFiniteTerminalEmission, AssignedNormalCloseTransduction,
        AssignedTerminalTransduction, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostCallDisposition, HostCallId, PortId,
    RequestId, ValueRef, ValueStorage,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TIME_WINDOW_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TimeWindowBack {
    duration: Option<ValueRef>,
    request: u32,
    pending: Option<RequestId>,
    slots: Vec<Vec<u8>>,
    lengths: Vec<usize>,
    count: usize,
    candidate: Vec<u8>,
    candidate_len: Option<usize>,
    encoder: PreparedLeafSequenceEncoder,
    emit_staged: bool,
    closing: bool,
    complete_after_emit: bool,
    abnormal_candidate: Option<CanonicalValue>,
    abnormal: Option<CanonicalValue>,
    output_maximum: u32,
    terminal: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for TimeWindowBack {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        Some(AssignedTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::FlushThenPropagate(
                AssignedFiniteTerminalEmission {
                    maximum_items: 1,
                    maximum_bytes: self.output_maximum,
                },
            ),
            abnormal: AssignedAbnormalTransduction::DomainSpecific {
                law: conduit_core::semantic_digest(
                    "conduit/kind-identity",
                    b"time/window/abnormal-cancels-boundary@1",
                ),
            },
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }

        if let Some(terminal) = self.abnormal {
            if let Some((request, outcome)) = io.host_completion() {
                if self.pending != Some(request)
                    || outcome.disposition != HostCallDisposition::Cancelled
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                {
                    return fail(940);
                }
                io.consume_host_completion()
                    .expect("observed time/window deadline cancellation");
                self.pending = None;
            }
            if self.pending.is_some() {
                return StepOutcome::Await;
            }
            self.release_duration(io);
            self.terminal = true;
            return StepOutcome::Abnormal {
                port: PortId(0),
                terminal,
            };
        }

        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return fail(941);
            }
            match outcome.disposition {
                HostCallDisposition::Completed => {
                    if self.count == 0 || !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if self.encode_window().is_err() {
                        return fail(942);
                    }
                    io.consume_host_completion()
                        .expect("observed time/window boundary completion");
                    io.send_prepared(PortId(0), self.encoder.encoded().len() as u32)
                        .expect("ready exact time/window output");
                    self.emit_staged = true;
                    return StepOutcome::Progress;
                }
                HostCallDisposition::Cancelled if self.closing => {
                    if self.count > 0 && !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if self.count > 0 {
                        if self.encode_window().is_err() {
                            return fail(943);
                        }
                        io.send_prepared(PortId(0), self.encoder.encoded().len() as u32)
                            .expect("ready final time/window output");
                        self.emit_staged = true;
                        self.complete_after_emit = true;
                    }
                    io.consume_host_completion()
                        .expect("observed closing time/window cancellation");
                    self.pending = None;
                    if self.count == 0 {
                        self.release_duration(io);
                        self.terminal = true;
                        return StepOutcome::Complete;
                    }
                    return StepOutcome::Progress;
                }
                _ => return fail(944),
            }
        }

        if let Some(terminal) = io.input_abnormal(PortId(0)) {
            io.consume_abnormal(PortId(0))
                .expect("present time/window abnormal terminal");
            self.abnormal_candidate = Some(terminal);
            if let Some(request) = self.pending {
                io.cancel_host_call(request)
                    .expect("cancel time/window boundary after abnormal terminal");
                return StepOutcome::Progress;
            }
            self.release_duration(io);
            self.terminal = true;
            return StepOutcome::Abnormal {
                port: PortId(0),
                terminal,
            };
        }

        if io.input_closed(PortId(0)) && !self.closing {
            io.consume_closed(PortId(0))
                .expect("observed time/window input closure");
            self.closing = true;
            if let Some(request) = self.pending {
                io.cancel_host_call(request)
                    .expect("cancel closing time/window boundary");
                return StepOutcome::Progress;
            }
            self.release_duration(io);
            self.terminal = true;
            return StepOutcome::Complete;
        }

        if self.complete_after_emit {
            self.release_duration(io);
            self.terminal = true;
            return StepOutcome::Complete;
        }

        if let Some(reference) = io.input(PortId(0)) {
            if self.closing || self.count >= self.slots.len() || self.candidate_len.is_some() {
                return StepOutcome::Await;
            }
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(945);
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > self.candidate.len() {
                return fail(946);
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_len = Some(bytes.len());
            io.consume(PortId(0)).expect("present time/window value");
            if self.pending.is_none() {
                let Some(duration) = self.duration else {
                    return fail(947);
                };
                let request = self.request.checked_add(1).map(RequestId);
                let Some(request) = request else {
                    return fail(948);
                };
                io.request_host_call(
                    request,
                    HostCallId(0),
                    BoundedValueRef::new(duration, 8)
                        .expect("time/window duration is exactly eight bytes"),
                )
                .expect("time/window monotonic Host Call");
                self.pending = Some(request);
            }
            return StepOutcome::Progress;
        }

        StepOutcome::Await
    }

    fn step_committed(&mut self) {
        if let Some(length) = self.candidate_len.take() {
            self.slots[self.count][..length].copy_from_slice(&self.candidate[..length]);
            self.lengths[self.count] = length;
            self.count += 1;
            if let Some(request) = self.pending {
                self.request = request.0;
            }
        }
        if self.emit_staged {
            self.emit_staged = false;
            self.count = 0;
            self.pending = None;
        }
        if let Some(terminal) = self.abnormal_candidate.take() {
            self.abnormal = Some(terminal);
            self.count = 0;
        }
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.emit_staged).then(|| self.encoder.encoded())
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        self.count < self.slots.len() && !self.closing && self.abnormal.is_none()
    }

    fn retains_host_call_input(&self, _: RequestId, value: ValueRef) -> bool {
        self.duration == Some(value)
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.count = 0;
        self.candidate_len = None;
        self.abnormal_candidate = None;
        self.abnormal = None;
        self.terminal = true;
    }
}

impl TimeWindowBack {
    pub(super) fn allocation_capacity(&self) -> usize {
        self.slots.iter().map(Vec::capacity).sum::<usize>()
            + self.lengths.capacity() * core::mem::size_of::<usize>()
            + self.candidate.capacity()
            + self.encoder.maximum_bytes() as usize
    }

    fn encode_window(&mut self) -> Result<(), ()> {
        let slots = &self.slots;
        let lengths = &self.lengths;
        let count = self.count;
        self.encoder
            .encode((0..count).map(|index| &slots[index][..lengths[index]]))
            .map(|_| ())
            .map_err(|_| ())
    }

    fn release_duration<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        if let Some(duration) = self.duration.take() {
            io.discard(duration).expect("release time/window duration");
        }
    }
}

fn exact_contracts(
    placement: &PlannedGear,
) -> Result<(&CheckedValueContract, &CheckedValueContract, u16), String> {
    let input = placement
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("value")))
        .ok_or("time/window placement has no exact input value contract")?;
    let output = placement
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("window")))
        .ok_or("time/window placement has no exact output value contract")?;
    let maximum_items = placement
        .semantic_contract
        .laws
        .iter()
        .find_map(|law| match law {
            conduit_core::KindSemanticLaw::Terminal(
                conduit_core::KindTerminalBehavior::TumblingProcessingTimeWindow { maximum_items },
            ) => Some(*maximum_items),
            _ => None,
        })
        .ok_or("time/window placement has no exact window law")?;
    Ok((&input.contract, &output.contract, maximum_items))
}

fn validate(
    placement: &PlannedGear,
) -> Result<(&CheckedValueContract, &CheckedValueContract, u16), String> {
    let (value, window, maximum_items) = exact_contracts(placement)?;
    let expected = conduit_semantic_catalog::time_window_semantic_contract(value, maximum_items)
        .map_err(str::to_string)?;
    let offer =
        conduit_std_offers::time_window_offer(value, maximum_items).map_err(str::to_string)?;
    if window.maximum_bytes as usize > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::TIME_WINDOW_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::TIME_WINDOW_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::TIME_WINDOW_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::TIME_WINDOW_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::TIME_WINDOW_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || placement.host_calls != offer.host_calls
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned time/window identity differs from its exact specialization".into());
    }
    Ok((value, window, maximum_items))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (_, window, _) = validate(placement)?;
    let value_bytes = window
        .maximum_bytes
        .checked_add(8)
        .ok_or("time/window prepared value budget overflows")?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes,
        host_requests: 1,
        sign_items: 128,
        maximum_value_bytes: window.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (value, window, maximum_items) = validate(placement)?;
    let duration_ms = timing_configuration::parse_window(placement)?;
    let duration = values
        .store(&encode_monotonic_duration(duration_ms))
        .map_err(|error| format!("store admitted time/window duration: {error:?}"))?;
    let maximum = value.maximum_bytes as usize;
    let slots = (0..maximum_items)
        .map(|_| vec![0; maximum])
        .collect::<Vec<_>>();
    let encoder = PreparedLeafSequenceEncoder::new(
        value.value_kind.clone(),
        value.maximum_bytes,
        maximum_items,
    )
    .map_err(|error| format!("prepare time/window encoder: {error:?}"))?;
    if encoder.maximum_bytes() != window.maximum_bytes {
        return Err("time/window prepared output differs from its checked contract".into());
    }
    Ok(InstalledBack::TimeWindow(Box::new(TimeWindowBack {
        duration: Some(duration),
        request: 0,
        pending: None,
        slots,
        lengths: vec![0; usize::from(maximum_items)],
        count: 0,
        candidate: vec![0; maximum],
        candidate_len: None,
        encoder,
        emit_staged: false,
        closing: false,
        complete_after_emit: false,
        abnormal_candidate: None,
        abnormal: None,
        output_maximum: window.maximum_bytes,
        terminal: false,
    })))
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

#[cfg(test)]
#[path = "time_window_back/tests.rs"]
mod tests;
