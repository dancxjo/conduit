//! Allocation-prepared runtime Back for collecting one closing Flow.

use alloc::{vec, vec::Vec};
use conduit_core::{CheckedValueContract, PreparedLeafSequenceEncoder};
use conduit_kernel::{
    scheduler::{AssignedTerminalTransduction, StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, Failure, FailureCode, PortId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowCollectPreparationError {
    InvalidSequenceContract,
    OutputContractMismatch { expected: u32, actual: u32 },
}

/// Collects at most one prepared number of values in arrival order.
///
/// All element and result storage is allocated by [`Self::prepare`]. An
/// overflow is an exact typed abnormal terminal selected during preparation;
/// it is neither execution failure nor pressure.
pub struct FlowCollectBack {
    slots: Vec<Vec<u8>>,
    lengths: Vec<usize>,
    count: usize,
    candidate: Vec<u8>,
    candidate_len: Option<usize>,
    encoder: PreparedLeafSequenceEncoder,
    overflow_terminal: CanonicalValue,
    output_pending: bool,
    output_staged: bool,
    complete_after_output: bool,
    terminal: bool,
}

impl FlowCollectBack {
    pub fn prepare(
        value: &CheckedValueContract,
        maximum_items: u16,
        output_maximum: u32,
        overflow_terminal: CanonicalValue,
    ) -> Result<Self, FlowCollectPreparationError> {
        let encoder = PreparedLeafSequenceEncoder::new(
            value.value_kind.clone(),
            value.maximum_bytes,
            maximum_items,
        )
        .map_err(|_| FlowCollectPreparationError::InvalidSequenceContract)?;
        if encoder.maximum_bytes() != output_maximum {
            return Err(FlowCollectPreparationError::OutputContractMismatch {
                expected: encoder.maximum_bytes(),
                actual: output_maximum,
            });
        }
        let element_maximum = value.maximum_bytes as usize;
        Ok(Self {
            slots: (0..maximum_items)
                .map(|_| vec![0; element_maximum])
                .collect(),
            lengths: vec![0; usize::from(maximum_items)],
            count: 0,
            candidate: vec![0; element_maximum],
            candidate_len: None,
            encoder,
            overflow_terminal,
            output_pending: false,
            output_staged: false,
            complete_after_output: false,
            terminal: false,
        })
    }

    /// Total heap capacity owned by this Back after preparation.
    pub fn allocation_capacity(&self) -> usize {
        self.slots.iter().map(Vec::capacity).sum::<usize>()
            + self.slots.capacity() * core::mem::size_of::<Vec<u8>>()
            + self.lengths.capacity() * core::mem::size_of::<usize>()
            + self.candidate.capacity()
            + self.encoder.maximum_bytes() as usize
    }

    fn encode_result(&mut self) -> Result<(), ()> {
        self.encoder
            .encode((0..self.count).map(|index| &self.slots[index][..self.lengths[index]]))
            .map(|_| ())
            .map_err(|_| ())
    }
}

impl<const PORTS: usize> StepBack<PORTS> for FlowCollectBack {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        // `flow/collect` turns a normally closing Flow into one Value. It does
        // not propagate a close onto that Value output, so the ordinary
        // Flow-to-Flow terminal-transduction profiles do not apply.
        None
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }

        if self.complete_after_output {
            self.terminal = true;
            return StepOutcome::Complete;
        }

        if self.output_pending {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send_prepared(PortId(0), self.encoder.encoded().len() as u32)
                .expect("ready exact flow/collect output");
            self.output_staged = true;
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) {
            if self.encode_result().is_err() {
                return fail(950);
            }
            io.consume_closed(PortId(0))
                .expect("present flow/collect normal close");
            self.output_pending = true;
            return StepOutcome::Progress;
        }

        if let Some(reference) = io.input(PortId(0)) {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(951);
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > self.candidate.len() {
                return fail(952);
            }
            io.consume(PortId(0))
                .expect("present flow/collect input value");
            if self.count == self.slots.len() {
                self.count = 0;
                self.terminal = true;
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal: self.overflow_terminal,
                };
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_len = Some(bytes.len());
            return StepOutcome::Progress;
        }

        StepOutcome::Await
    }

    fn step_committed(&mut self) {
        if let Some(length) = self.candidate_len.take() {
            self.slots[self.count][..length].copy_from_slice(&self.candidate[..length]);
            self.lengths[self.count] = length;
            self.count += 1;
        }
        if self.output_staged {
            self.output_staged = false;
            self.output_pending = false;
            self.complete_after_output = true;
            self.count = 0;
        }
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.output_staged).then(|| self.encoder.encoded())
    }

    fn cancel(&mut self) {
        self.count = 0;
        self.candidate_len = None;
        self.output_pending = false;
        self.output_staged = false;
        self.complete_after_output = false;
        self.terminal = true;
    }
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

#[cfg(test)]
#[path = "flow_collect_back_tests.rs"]
mod tests;
