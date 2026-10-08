//! Bounded development output observation. Source owns Native admission.
//! All row and staging buffers are prepared before scheduler Play.
use super::*;
#[derive(Clone)]
pub(super) struct OutputPool {
    pub(super) received: Rc<std::cell::RefCell<Vec<Vec<u8>>>>,
    spare: Rc<std::cell::RefCell<Vec<Vec<u8>>>>,
    expected: usize,
}
impl OutputPool {
    pub(super) fn prepare(expected: usize) -> Self {
        assert!(expected <= 256, "explicit finite diagnostic capacity");
        Self {
            received: Rc::new(std::cell::RefCell::new(Vec::with_capacity(expected))),
            spare: Rc::new(std::cell::RefCell::new(
                (0..expected).map(|_| Vec::with_capacity(16384)).collect(),
            )),
            expected,
        }
    }
}
pub(super) struct PrimarySink {
    pool: OutputPool,
    staging: Vec<u8>,
    ready: bool,
    cancelled: bool,
}
impl PrimarySink {
    pub(super) fn prepare(pool: OutputPool) -> Self {
        Self {
            pool,
            staging: Vec::with_capacity(16384),
            ready: false,
            cancelled: false,
        }
    }
}
impl StepBack<PORTS> for PrimarySink {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.ready = false;
        if self.cancelled {
            return fail(conduit_kernel::FailureCode::Cancelled);
        }
        let Some(input) = bytes.input(KPort(0)) else {
            return if io.input_closed(KPort(0)) {
                StepOutcome::Complete
            } else {
                StepOutcome::Await
            };
        };
        if input.len() > 16384 || self.pool.received.borrow().len() >= self.pool.expected {
            return fail(conduit_kernel::FailureCode::InvalidInput);
        }
        self.staging.clear();
        self.staging.extend_from_slice(input);
        self.ready = true;
        io.consume(KPort(0)).unwrap();
        if self.pool.received.borrow().len() + 1 == self.pool.expected {
            StepOutcome::Complete
        } else {
            StepOutcome::Progress
        }
    }
    fn step_committed(&mut self) {
        if self.ready {
            self.ready = false;
            let mut spare = self
                .pool
                .spare
                .borrow_mut()
                .pop()
                .expect("admitted output count");
            core::mem::swap(&mut spare, &mut self.staging);
            self.pool.received.borrow_mut().push(spare);
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.ready = false;
    }
}
fn fail(code: conduit_kernel::FailureCode) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure { code, detail: 967 })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(input: &[u8]) -> (StepIo<PORTS>, StepInputBytes<'_, PORTS>) {
        let mut references = [None; PORTS];
        references[0] = Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: input.len() as u32,
        });
        let mut inputs = [None; PORTS];
        inputs[0] = Some(input);
        (
            StepIo::test_frame(references, [false; PORTS], [None; PORTS], None, 16384),
            StepInputBytes::test_frame(inputs, None),
        )
    }
    #[test]
    fn primary_output_buffers_commit_exactly_without_allocating() {
        let pool = OutputPool::prepare(2);
        let mut sink = PrimarySink::prepare(pool.clone());
        for (index, input) in [
            b"first Source publication".as_slice(),
            b"second Source publication",
        ]
        .into_iter()
        .enumerate()
        {
            let (mut io, bytes) = frame(input);
            let (outcome, allocations) =
                crate::allocation_probe::measure(|| sink.step(&mut io, &bytes));
            assert_eq!(allocations, 0);
            assert_eq!(
                outcome,
                if index == 1 {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                }
            );
            assert_eq!(pool.received.borrow().len(), index);
            let (_, allocations) = crate::allocation_probe::measure(|| sink.step_committed());
            assert_eq!(allocations, 0);
            assert_eq!(pool.received.borrow()[index], input);
        }
    }
    #[test]
    fn primary_output_cancel_and_oversize_publish_nothing() {
        let pool = OutputPool::prepare(1);
        let mut sink = PrimarySink::prepare(pool.clone());
        let (mut io, bytes) = frame(b"provisional publication");
        assert_eq!(sink.step(&mut io, &bytes), StepOutcome::Complete);
        sink.cancel();
        sink.step_committed();
        assert!(pool.received.borrow().is_empty());
        let mut sink = PrimarySink::prepare(pool.clone());
        let oversized = vec![0; 16385];
        let (mut io, bytes) = frame(&oversized);
        let (outcome, allocations) =
            crate::allocation_probe::measure(|| sink.step(&mut io, &bytes));
        assert_eq!(allocations, 0);
        assert!(matches!(outcome, StepOutcome::Fail(_)));
        assert!(!io.test_consumed(KPort(0)));
        assert!(pool.received.borrow().is_empty());
    }
}
