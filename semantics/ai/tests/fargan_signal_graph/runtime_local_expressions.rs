//! Timing adapter for the explicitly selected shared computational owner.
//! Its execution time is inside kernel time; hosted expression time is outside.
use super::*;
use conduit_kernel::scheduler::{
    prepared_session::ReusableStepBack, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::PortId;
pub(super) struct TimedExpression {
    pub back: conduit_semantic_catalog::PreparedExpressionBack,
    pub timing: Rc<std::cell::RefCell<WorkTiming>>,
}
impl StepBack<PORTS> for TimedExpression {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let start = Instant::now();
        let result = self.back.step(io, inputs);
        let mut timing = self.timing.borrow_mut();
        timing.local_expressions += start.elapsed();
        if result == StepOutcome::Progress {
            timing.local_expression_calls += 1;
        }
        result
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        <conduit_semantic_catalog::PreparedExpressionBack as StepBack<PORTS>>::prepared_output(
            &self.back, port,
        )
    }
    fn step_committed(&mut self) {
        <conduit_semantic_catalog::PreparedExpressionBack as StepBack<PORTS>>::step_committed(
            &mut self.back,
        );
    }
    fn cancel(&mut self) {
        <conduit_semantic_catalog::PreparedExpressionBack as StepBack<PORTS>>::cancel(
            &mut self.back,
        );
    }
}
impl ReusableStepBack<PORTS> for TimedExpression {
    type Next = ();
    fn preflight_next(&self, next: &(), values: &dyn ValueStorage) -> bool {
        <conduit_semantic_catalog::PreparedExpressionBack as ReusableStepBack<PORTS>>::preflight_next(&self.back, next, values)
    }
    fn begin_next(&mut self, next: ()) {
        <conduit_semantic_catalog::PreparedExpressionBack as ReusableStepBack<PORTS>>::begin_next(
            &mut self.back,
            next,
        );
    }
}
