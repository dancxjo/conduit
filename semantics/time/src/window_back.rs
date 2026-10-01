//! Allocation-prepared kernel operation for one bounded processing-time window.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{CheckedValueContract, PreparedLeafSequenceEncoder};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedFiniteTerminalEmission, AssignedNormalCloseTransduction,
        AssignedTerminalTransduction, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostCallDisposition, HostCallId, PortId,
    RequestId, ValueRef,
};

pub struct ProcessingTimeWindowBack {
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

impl<const PORTS: usize> StepBack<PORTS> for ProcessingTimeWindowBack {
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

impl ProcessingTimeWindowBack {
    pub fn prepare(
        duration: ValueRef,
        value: &CheckedValueContract,
        output_maximum: u32,
        maximum_items: u16,
    ) -> Result<Self, String> {
        let maximum = value.maximum_bytes as usize;
        let encoder = PreparedLeafSequenceEncoder::new(
            value.value_kind.clone(),
            value.maximum_bytes,
            maximum_items,
        )
        .map_err(|error| format!("prepare time/window encoder: {error:?}"))?;
        if encoder.maximum_bytes() != output_maximum {
            return Err("time/window prepared output differs from its checked contract".into());
        }
        Ok(Self {
            duration: Some(duration),
            request: 0,
            pending: None,
            slots: (0..maximum_items).map(|_| vec![0; maximum]).collect(),
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
            output_maximum,
            terminal: false,
        })
    }

    pub fn allocation_capacity(&self) -> usize {
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

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

#[cfg(test)]
#[path = "window_back_tests.rs"]
mod tests;
