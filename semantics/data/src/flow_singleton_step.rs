//! One explicit Value-to-closing-Flow transfer, with no allocation or state.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub struct FlowSingletonStep;
impl<const PORTS: usize> StepBack<PORTS> for FlowSingletonStep {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let Some(value) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 2,
                })
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err() || io.send(PortId(0), value).is_err() {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 1,
            });
        }
        // The scheduler commits the value transfer and normal closure together.
        // Withholding a failed I/O commit changes no private Back state.
        StepOutcome::Complete
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueRef;
    #[test]
    fn pressure_keeps_input_and_value_transfer_completes_atomically() {
        let value = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 8,
        };
        let mut back = FlowSingletonStep;
        let mut pressure = StepIo::test_frame([Some(value)], [false], [None], None, 8);
        assert_eq!(
            back.step(&mut pressure, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Await
        );
        assert!(!pressure.test_consumed(PortId(0)));
        let mut ready = StepIo::test_frame([Some(value)], [false], [Some(8)], None, 8);
        assert_eq!(
            back.step(&mut ready, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert!(ready.test_consumed(PortId(0)));
        assert_eq!(ready.test_output(PortId(0)), Some(value));
        assert_eq!(core::mem::size_of::<FlowSingletonStep>(), 0);
    }
    #[test]
    fn missing_required_value_is_refused_instead_of_inventing_empty_flow() {
        let mut back = FlowSingletonStep;
        let mut closed = StepIo::test_frame([None], [true], [Some(8)], None, 8);
        assert!(matches!(
            back.step(&mut closed, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Fail(_)
        ));
        assert!(closed.test_output(PortId(0)).is_none());
    }
}
