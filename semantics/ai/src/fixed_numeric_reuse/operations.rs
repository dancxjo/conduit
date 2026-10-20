//! Preserve prepared operation representations across admitted Value invocations.
use super::*;
use conduit_kernel::scheduler::prepared_session::ReusableStepBack;

impl Invocation {
    fn reusable_value(&self) -> bool {
        self.finished
            && !self.staged
            && !self.cancelled
            && !self.flow
            && self.committed_frames < u64::MAX
    }
}

impl<const WIDTH: usize, const PORTS: usize> ReusableStepBack<PORTS> for FixedTanhBack<WIDTH> {
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn conduit_kernel::ValueStorage) -> bool {
        self.state.reusable_value()
    }
    fn begin_next(&mut self, _: ()) {
        self.state.finished = false;
    }
}

impl<const LEFT: usize, const RIGHT: usize, const OUTPUT: usize, const PORTS: usize>
    ReusableStepBack<PORTS> for FixedConcatenateBack<LEFT, RIGHT, OUTPUT>
{
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn conduit_kernel::ValueStorage) -> bool {
        self.state.reusable_value()
    }
    fn begin_next(&mut self, _: ()) {
        self.state.finished = false;
    }
}

impl<const ROWS: usize, const WIDTH: usize, const PORTS: usize> ReusableStepBack<PORTS>
    for FixedEmbeddingBack<'_, ROWS, WIDTH>
{
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn conduit_kernel::ValueStorage) -> bool {
        self.state.reusable_value()
    }
    fn begin_next(&mut self, _: ()) {
        self.state.finished = false;
    }
}
