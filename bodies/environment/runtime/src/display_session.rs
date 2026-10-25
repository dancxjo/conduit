//! Explicit reuse of pure output computations and the finite electrical sink.
//! Stateful sampler, clock and publication Backs cannot use this profile.
use crate::{appliance::ApplianceBack, display_output::DisplayBack, CaptureBack};
use conduit_computation::{PreparedExpressionBack, StructuredSelectorBack};
use conduit_kernel::{scheduler::prepared_session::ReusableStepBack, ValueStorage};

impl<const P: usize> ReusableStepBack<P> for CaptureBack {
    type Next = ();
    fn preflight_next(&self, next: &(), values: &dyn ValueStorage) -> bool {
        match self {
            Self::Expression(back) => {
                <PreparedExpressionBack as ReusableStepBack<P>>::preflight_next(back, next, values)
            }
            Self::Selector(back) => {
                <StructuredSelectorBack as ReusableStepBack<P>>::preflight_next(back, next, values)
            }
            Self::Pulse(_) | Self::Observation(_) => false,
        }
    }
    fn begin_next(&mut self, next: ()) {
        match self {
            Self::Expression(back) => {
                <PreparedExpressionBack as ReusableStepBack<P>>::begin_next(back, next)
            }
            Self::Selector(back) => {
                <StructuredSelectorBack as ReusableStepBack<P>>::begin_next(back, next)
            }
            Self::Pulse(_) | Self::Observation(_) => {}
        }
    }
}
impl<const P: usize> ReusableStepBack<P> for ApplianceBack {
    type Next = ();
    fn preflight_next(&self, next: &(), values: &dyn ValueStorage) -> bool {
        match self {
            Self::Native(back) => {
                <CaptureBack as ReusableStepBack<P>>::preflight_next(back, next, values)
            }
            Self::Tick(_) | Self::Trigger(_) | Self::Publication(_) => false,
        }
    }
    fn begin_next(&mut self, next: ()) {
        if let Self::Native(back) = self {
            <CaptureBack as ReusableStepBack<P>>::begin_next(back, next);
        }
    }
}
impl<const P: usize> ReusableStepBack<P> for DisplayBack {
    type Next = ();
    fn preflight_next(&self, next: &(), values: &dyn ValueStorage) -> bool {
        match self {
            Self::Software(back) => {
                <ApplianceBack as ReusableStepBack<P>>::preflight_next(back, next, values)
            }
            Self::Write(back) => {
                <conduit_tm1637_wire::WriteBack as ReusableStepBack<P>>::preflight_next(
                    back, next, values,
                )
            }
        }
    }
    fn begin_next(&mut self, next: ()) {
        match self {
            Self::Software(back) => <ApplianceBack as ReusableStepBack<P>>::begin_next(back, next),
            Self::Write(back) => {
                <conduit_tm1637_wire::WriteBack as ReusableStepBack<P>>::begin_next(back, next)
            }
        }
    }
}
