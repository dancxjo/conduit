//! Bounded FIFO pairing whose two finite inputs retain independent lifetimes.
use super::*;

/// A distinct close law; never substitute this for shortest-stream or feedback
/// pairing merely because their payload types agree. Selection must validate
/// the exact semantic/implementation contract before constructing the Back.
pub struct FlowZipExactBack {
    inner: FlowZipBack,
    closed: [bool; 2],
    staged_close: Option<usize>,
    cancelled: bool,
}
impl FlowZipExactBack {
    fn refuse_unmatched<const P: usize>(&self, io: &mut StepIo<P>) -> StepOutcome {
        // Consume exposed normal terminal truth even on refusal. The kernel
        // observes Fail, so this cannot become successful output closure.
        for side in 0..2 {
            if !self.closed[side] && io.input_closed(PortId(side as u16)) {
                io.consume_closed(PortId(side as u16))
                    .expect("unequal exact stream close");
                break;
            }
        }
        fail(918)
    }
    pub fn prepare(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            inner: FlowZipBack::prepare_typed_finite(left, left_type, right, right_type)?,
            closed: [false; 2],
            staged_close: None,
            cancelled: false,
        })
    }
}
impl<const P: usize> StepBack<P> for FlowZipExactBack {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; P] {
        let mut contracts = [None; P];
        for (side, contract) in contracts.iter_mut().enumerate().take(2) {
            *contract = Some(AssignedTerminalTransduction {
                input: PortId(side as u16),
                output: PortId(0),
                normal_close: AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(
                    AssignedFiniteTerminalEmission {
                        maximum_items: 1,
                        maximum_bytes: self.inner.output_maximum,
                    },
                ),
                abnormal: AssignedAbnormalTransduction::NotAccepted,
                cancellation: AssignedCancellationTransduction::NotCancellable,
            });
        }
        contracts
    }
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Complete;
        }
        for side in 0..2 {
            if io.input_abnormal(PortId(side)).is_some() {
                return fail(917);
            }
        }
        // A committed pending value on a closed side is still available for
        // pairing. Only an exhausted closed side proves cardinality mismatch.
        if (self.closed[0] && self.inner.left_len.is_none() && self.inner.right_len.is_some())
            || (self.closed[1] && self.inner.right_len.is_none() && self.inner.left_len.is_some())
        {
            return self.refuse_unmatched(io);
        }
        if self.inner.left_len.is_some() && self.inner.right_len.is_some() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(length) = self.inner.encode_pair() else {
                return fail(912);
            };
            for side in 0..2 {
                if !self.closed[side] && io.input_closed(PortId(side as u16)) {
                    io.consume_closed(PortId(side as u16))
                        .expect("exact close with final pair");
                    self.staged_close = Some(side);
                    break;
                }
            }
            self.inner.output_staged = true;
            io.send_prepared(PortId(0), length as u32)
                .expect("admitted exact pair output");
            return StepOutcome::Progress;
        }
        for side in 0..2 {
            if !self.closed[side] && io.input_closed(PortId(side as u16)) {
                // Unequal cardinality is a failure, never a successful close.
                let pending = if side == 0 {
                    self.inner.right_len
                } else {
                    self.inner.left_len
                };
                let own = if side == 0 {
                    self.inner.left_len
                } else {
                    self.inner.right_len
                };
                if own.is_none() && pending.is_some() {
                    return self.refuse_unmatched(io);
                }
                io.consume_closed(PortId(side as u16))
                    .expect("exact input close");
                self.staged_close = Some(side);
                return if self.closed[1 - side] {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        if self.closed == [true; 2] {
            return StepOutcome::Complete;
        }
        // No unconsumed terminal is exposed here. Reuse the existing prepared
        // byte validation/staging; this path neither allocates nor grows queues.
        self.inner.step(io, inputs)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if self.cancelled {
            None
        } else {
            <FlowZipBack as StepBack<P>>::prepared_output(&self.inner, port)
        }
    }
    fn step_committed(&mut self) {
        <FlowZipBack as StepBack<P>>::step_committed(&mut self.inner);
        if let Some(side) = self.staged_close.take() {
            self.closed[side] = true;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged_close = None;
        <FlowZipBack as StepBack<P>>::cancel(&mut self.inner);
    }
}
