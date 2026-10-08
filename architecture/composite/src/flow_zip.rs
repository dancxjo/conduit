//! Allocation-prepared, host-independent realization of exact `flow/zip`.
use crate::prelude::*;
use conduit_core::{
    CheckedValueContract, PreparedTuplePairEncoder, PreparedTypedTuplePairEncoder,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
};
mod encoder;
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedFiniteTerminalEmission, AssignedNormalCloseTransduction,
        AssignedTerminalTransduction, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};
use encoder::PairEncoder;

pub struct FlowZipBack {
    left: Vec<u8>,
    left_len: Option<usize>,
    right: Vec<u8>,
    right_len: Option<usize>,
    candidate: Vec<u8>,
    candidate_side: Option<(usize, usize)>,
    encoder: PairEncoder,
    output_staged: bool,
    finish_after_commit: bool,
    closing: bool,
    terminal: bool,
    output_maximum: u32,
    finite: bool,
    feedback: bool,
    return_owed: bool,
    right_closed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for FlowZipBack {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut contracts = [None; PORTS];
        let flush =
            AssignedNormalCloseTransduction::FlushThenPropagate(AssignedFiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: self.output_maximum,
            });
        for (input, contract) in contracts.iter_mut().enumerate().take(2) {
            *contract = Some(AssignedTerminalTransduction {
                input: PortId(input as u16),
                output: PortId(0),
                normal_close: flush,
                abnormal: if self.finite {
                    AssignedAbnormalTransduction::NotAccepted
                } else {
                    AssignedAbnormalTransduction::PropagateAfterDrain
                },
                cancellation: AssignedCancellationTransduction::NotCancellable,
            });
        }
        contracts
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }
        // A finite close discards already-admitted unmatched queue entries,
        // one bounded consumption per step, before retiring this operation.
        if self.closing {
            for port in [PortId(0), PortId(1)] {
                if io.input(port).is_some() {
                    io.consume(port).expect("queued unmatched finite zip value");
                    return StepOutcome::Progress;
                }
            }
            self.terminal = true;
            return StepOutcome::Complete;
        }

        // Fault is not normal close: never emit a buffered pair as a side
        // effect of abnormal terminal truth.
        for port in [PortId(0), PortId(1)] {
            if let Some(terminal) = io.input_abnormal(port) {
                if self.finite {
                    return fail(917);
                }
                io.consume_abnormal(port)
                    .expect("present flow/zip abnormal terminal");
                self.terminal = true;
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal,
                };
            }
        }

        // Ordinary zip closure flushes only an already formed pair. The
        // feedback profile instead waits for the declared state return.
        for port in [PortId(0), PortId(1)] {
            if io.input_closed(port) {
                if self.feedback && port == PortId(1) {
                    io.consume_closed(port).expect("feedback event close");
                    self.right_closed = true;
                    return StepOutcome::Progress;
                }
                let pair_ready = self.left_len.is_some() && self.right_len.is_some();
                if pair_ready && !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume_closed(port)
                    .expect("present flow/zip normal close");
                if pair_ready {
                    let output_len = self.encode_pair().unwrap_or(0);
                    if output_len == 0 {
                        return fail(911);
                    }
                    self.output_staged = true;
                    self.finish_after_commit = true;
                    io.send_prepared(PortId(0), output_len as u32)
                        .expect("ready admitted flow/zip output");
                    return StepOutcome::Progress;
                }
                self.left_len = None;
                self.right_len = None;
                if self.finite {
                    for other in [PortId(0), PortId(1)] {
                        if io.input(other).is_some() {
                            io.consume(other)
                                .expect("queued unmatched finite zip value");
                            self.closing = true;
                            return StepOutcome::Progress;
                        }
                    }
                }
                self.terminal = true;
                return StepOutcome::Complete;
            }
        }

        if self.feedback && self.right_closed && self.right_len.is_none() && !self.return_owed {
            self.left_len = None;
            if io.input(PortId(0)).is_some() {
                io.consume(PortId(0)).expect("final feedback state");
                self.closing = true;
                return StepOutcome::Progress;
            }
            self.terminal = true;
            return StepOutcome::Complete;
        }

        if self.left_len.is_some() && self.right_len.is_some() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let output_len = self.encode_pair().unwrap_or(0);
            if output_len == 0 {
                return fail(912);
            }
            self.output_staged = true;
            io.send_prepared(PortId(0), output_len as u32)
                .expect("ready admitted flow/zip output");
            return StepOutcome::Progress;
        }

        for (side, port, pending, maximum) in [
            (0, PortId(0), self.left_len, self.left.len()),
            (1, PortId(1), self.right_len, self.right.len()),
        ] {
            if pending.is_some() {
                continue;
            }
            let Some(reference) = io.input(port) else {
                continue;
            };
            let Some(bytes) = inputs.input(port) else {
                return fail(913 + side as u16);
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > maximum {
                return fail(915 + side as u16);
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_side = Some((side, bytes.len()));
            io.consume(port).expect("present flow/zip input");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.output_staged).then(|| {
            // The last encode length is the exact staged length. The encoder
            // exposes it through the prepared slice without allocating.
            self.encoder.encoded()
        })
    }

    fn step_committed(&mut self) {
        if let Some((side, len)) = self.candidate_side.take() {
            if side == 0 {
                self.left[..len].copy_from_slice(&self.candidate[..len]);
                self.left_len = Some(len);
                self.return_owed = false;
            } else {
                self.right[..len].copy_from_slice(&self.candidate[..len]);
                self.right_len = Some(len);
            }
        }
        if self.output_staged {
            self.return_owed = self.feedback;
            self.left_len = None;
            self.right_len = None;
            self.output_staged = false;
        }
        if self.finish_after_commit {
            self.finish_after_commit = false;
            if self.finite {
                self.closing = true;
            } else {
                self.terminal = true;
            }
        }
    }

    fn cancel(&mut self) {
        self.left_len = None;
        self.right_len = None;
        self.candidate_side = None;
        self.terminal = true;
    }
}

impl FlowZipBack {
    fn encode_pair(&mut self) -> Option<usize> {
        let left = &self.left[..self.left_len?];
        let right = &self.right[..self.right_len?];
        self.encoder.encode(left, right).ok().map(<[u8]>::len)
    }
}

impl FlowZipBack {
    /// Prepare typed pairing for payload-only finite streams. Normal closure
    /// flushes at most one already-formed pair, exactly as the shared zip law.
    pub fn prepare_typed_finite(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let mut back = Self::prepare_typed(left, left_type, right, right_type)?;
        back.finite = true;
        Ok(back)
    }

    /// Pair events with successive returned state generations. Event closure
    /// drains a pending event and waits for the final published pair's return.
    pub fn prepare_typed_feedback(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let mut back = Self::prepare_typed_finite(left, left_type, right, right_type)?;
        back.feedback = true;
        back.return_owed = true;
        Ok(back)
    }

    /// Prepare all pair buffers before play. The caller must validate the
    /// selected semantic and implementation contracts separately.
    pub fn prepare_typed(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let transport_kind = |ty: &StructuredInfoType| match ty.shape() {
            StructuredInfoTypeShape::Leaf(kind) => Ok(kind.clone()),
            _ => ty.profile().map(|profile| profile.value_kind().clone()),
        };
        if transport_kind(&left_type)? != left.value_kind
            || transport_kind(&right_type)? != right.value_kind
        {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let encoder = PreparedTypedTuplePairEncoder::new(
            left_type,
            left.maximum_bytes,
            right_type,
            right.maximum_bytes,
        )?;
        Ok(Self {
            left: vec![0; left.maximum_bytes as usize],
            left_len: None,
            right: vec![0; right.maximum_bytes as usize],
            right_len: None,
            candidate: vec![0; left.maximum_bytes.max(right.maximum_bytes) as usize],
            candidate_side: None,
            output_maximum: encoder.maximum_bytes(),
            encoder: PairEncoder::Typed(Box::new(encoder)),
            output_staged: false,
            finish_after_commit: false,
            closing: false,
            terminal: false,
            finite: false,
            feedback: false,
            return_owed: false,
            right_closed: false,
        })
    }

    /// Prepare primitive-leaf pairing with separately validated selected contracts.
    pub fn prepare(
        left: &CheckedValueContract,
        right: &CheckedValueContract,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        let encoder = PreparedTuplePairEncoder::new(
            left.value_kind.clone(),
            left.maximum_bytes,
            right.value_kind.clone(),
            right.maximum_bytes,
        )?;
        Ok(Self {
            left: vec![0; left.maximum_bytes as usize],
            left_len: None,
            right: vec![0; right.maximum_bytes as usize],
            right_len: None,
            candidate: vec![0; left.maximum_bytes.max(right.maximum_bytes) as usize],
            candidate_side: None,
            output_maximum: encoder.maximum_bytes(),
            encoder: PairEncoder::Leaf(Box::new(encoder)),
            output_staged: false,
            finish_after_commit: false,
            closing: false,
            terminal: false,
            finite: false,
            feedback: false,
            return_owed: false,
            right_closed: false,
        })
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(test)]
mod tests;

mod storage;
