//! Shared finite Flow-to-final-Value normalized-pattern selection.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};

pub struct FinalNormalizedPatternBack {
    latest: Option<ValueRef>,
    accepted: u64,
    maximum: u64,
}

impl<const PORTS: usize> StepBack<PORTS> for FinalNormalizedPatternBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if self.accepted >= self.maximum {
                return step_fail(FailureCode::StorageExhausted, 1);
            }
            let retained = io
                .take_input(PortId(0))
                .expect("present final-pattern input");
            debug_assert_eq!(retained, value);
            if let Some(previous) = self.latest.replace(retained) {
                io.discard(previous)
                    .expect("one replaced final-pattern value per Step");
            }
            self.accepted += 1;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            let Some(value) = self.latest else {
                return step_fail(FailureCode::InvalidInput, 2);
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_closed(PortId(0))
                .expect("observed final-pattern closure");
            io.send(PortId(0), value)
                .expect("ready final-pattern output");
            self.latest = None;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.latest = None;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl FinalNormalizedPatternBack {
    pub fn new(maximum: u64) -> Self {
        Self {
            latest: None,
            accepted: 0,
            maximum,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(maximum: u64) -> FinalNormalizedPatternBack {
        FinalNormalizedPatternBack {
            latest: None,
            accepted: 0,
            maximum,
        }
    }

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: 1,
        }
    }

    fn input(
        operation: &mut FinalNormalizedPatternBack,
        value: ValueRef,
    ) -> (StepOutcome, StepIo<1>) {
        let mut io = StepIo::test_frame([Some(value)], [false], [Some(1)], None, 2);
        let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
        (outcome, io)
    }

    #[test]
    fn step_preserves_replacement_release_and_final_emission() {
        let mut operation = FinalNormalizedPatternBack::new(2);
        let first = value(0);
        let second = value(1);
        for value in [first, second] {
            let (outcome, io) = input(&mut operation, value);
            assert_eq!(outcome, StepOutcome::Progress);
            assert_eq!(
                io.test_discards().iter().flatten().copied().next(),
                (value == second).then_some(first)
            );
        }
        let mut io = StepIo::test_frame([None], [true], [Some(1)], None, 2);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert_eq!(io.test_output(PortId(0)), Some(second));
    }

    #[test]
    fn empty_and_over_bound_flows_fail_distinctly() {
        let mut empty = operation(1);
        let mut io = StepIo::test_frame([None], [true], [Some(1)], None, 2);
        assert!(matches!(
            empty.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 2
            })
        ));
        let mut operation = operation(0);
        assert!(matches!(
            input(&mut operation, value(0)).0,
            StepOutcome::Fail(Failure {
                code: FailureCode::StorageExhausted,
                detail: 1
            })
        ));
    }
}
