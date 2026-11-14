//! One transactional Zip Step; pending sides remain owned between Steps.
use super::*;
impl FlowZipBack {
    pub(super) fn step_inner<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        mut output: Option<&mut [u8]>,
    ) -> StepOutcome {
        self.output_staged = false;
        self.finish_after_commit = false;
        self.candidate_side = None;
        if self.terminal {
            return StepOutcome::Complete;
        }
        // A finite close discards already-admitted unmatched queue entries,
        // one bounded consumption per step, before retiring this operation.
        if self.closing {
            // LEFT-first feedback closure cannot retire from an empty queue
            // snapshot: the finite event producer may still publish later.
            if self.drain_right_until_close && !self.right_closed {
                if io.input_abnormal(PortId(1)).is_some() {
                    return fail(917);
                }
                if io.input_closed(PortId(1)) {
                    io.consume_closed(PortId(1))
                        .expect("remaining feedback event close");
                    self.right_closed = true;
                    return StepOutcome::Progress;
                }
            }
            for port in [PortId(0), PortId(1)] {
                if io.input(port).is_some() {
                    io.consume(port).expect("queued unmatched finite zip value");
                    return StepOutcome::Progress;
                }
            }
            if self.drain_right_until_close && !self.right_closed {
                return StepOutcome::Await;
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
                let output_len = if pair_ready {
                    let Some(length) = self.encode_pair_into(output.as_deref_mut()) else {
                        return fail(911);
                    };
                    if !io.output_ready_for(PortId(0), length as u32) {
                        return StepOutcome::Await;
                    }
                    Some(length)
                } else {
                    None
                };
                io.consume_closed(port)
                    .expect("present flow/zip normal close");
                if self.feedback && port == PortId(0) && !self.right_closed {
                    self.drain_right_until_close = true;
                }
                if let Some(output_len) = output_len {
                    self.output_staged = true;
                    self.finish_after_commit = true;
                    Self::stage_output(io, output_len, output.is_some());
                    return StepOutcome::Progress;
                }
                self.left_len = None;
                self.right_len = None;
                if self.drain_right_until_close {
                    self.closing = true;
                    return StepOutcome::Progress;
                }
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
            let output_len = self.encode_pair_into(output.as_deref_mut()).unwrap_or(0);
            if output_len == 0 {
                return fail(912);
            }
            if !io.output_ready_for(PortId(0), output_len as u32) {
                return StepOutcome::Await;
            }
            self.output_staged = true;
            Self::stage_output(io, output_len, output.is_some());
            return StepOutcome::Progress;
        }

        for (side, port, pending, maximum) in [
            (
                0,
                PortId(0),
                self.left_len,
                self.encoder.logical_maximum(0, self.left.len()),
            ),
            (
                1,
                PortId(1),
                self.right_len,
                self.encoder.logical_maximum(1, self.right.len()),
            ),
        ] {
            if pending.is_some() {
                continue;
            }
            let Some(reference) = io.input(port) else {
                continue;
            };
            let length = match self.copy_input(inputs, port, reference.byte_len as usize, maximum) {
                Ok(length) => length,
                Err(detail) => return fail(detail),
            };
            // Only an empty slot may stage input. Its length remains absent
            // until commit, so these bytes cannot form an observable pair.
            self.candidate_side = Some((side, length));
            io.consume(port).expect("present flow/zip input");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
}
