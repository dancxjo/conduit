//! Explicit bounded rearm inputs for the ordinary fixture endpoints.
use super::*;
use conduit_kernel::{scheduler::prepared_session::ReusableStepBack, ValueStorage};
pub(crate) enum Next {
    Source(ValueRef),
    Operation,
    Sink(Vec<u8>),
    Inactive,
}
impl<B: ReusableStepBack<PORTS, Next = ()> + ?Sized> ReusableStepBack<PORTS> for TimedNumeric<B> {
    type Next = ();
    fn preflight_next(&self, next: &(), values: &dyn ValueStorage) -> bool {
        self.back.preflight_next(next, values)
    }
    fn begin_next(&mut self, next: ()) {
        self.back.begin_next(next);
    }
}
impl ReusableStepBack<PORTS> for Driver {
    type Next = Next;
    fn preflight_next(&self, next: &Next, values: &dyn ValueStorage) -> bool {
        match (self, next) {
            (Self::Source { sent, staged, .. }, Next::Source(value)) => {
                *sent && !staged && values.get(*value).is_ok()
            }
            (Self::ReusableOperation(back), Next::Operation) => back.preflight_next(&(), values),
            (Self::Sink { received, staged }, Next::Sink(bytes)) => {
                staged.is_none() && received.borrow().is_none() && bytes.capacity() == 16384
            }
            (Self::Inactive, Next::Inactive) => true,
            _ => false,
        }
    }
    fn begin_next(&mut self, next: Next) {
        match (self, next) {
            (
                Self::Source {
                    reference,
                    sent,
                    staged,
                },
                Next::Source(value),
            ) => {
                *reference = value;
                *sent = false;
                *staged = false;
            }
            (Self::ReusableOperation(back), Next::Operation) => back.begin_next(()),
            (Self::Sink { staged, .. }, Next::Sink(mut bytes)) => {
                bytes.clear();
                *staged = Some(bytes);
            }
            (Self::Inactive, Next::Inactive) => {}
            _ => unreachable!("all Back preflights passed"),
        }
    }
}
