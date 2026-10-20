//! The existing linear resource specimen can opt into the production session.
use super::*;
use conduit_kernel::scheduler::prepared_session::ReusableStepBack;

pub(super) enum Next {
    Source(ValueRef),
    Linear,
    Sink(Vec<u8>),
}
impl ReusableStepBack<PORTS> for Driver<'_> {
    type Next = Next;
    fn preflight_next(&self, next: &Next, values: &dyn ValueStorage) -> bool {
        match (self, next) {
            (Self::Source(_, sent), Next::Source(value)) => *sent && values.get(*value).is_ok(),
            (Self::Linear(back), Next::Linear) => <FixedLinearBack<80, 1> as ReusableStepBack<
                PORTS,
            >>::preflight_next(back, &(), values),
            (Self::Sink(seen, expected), Next::Sink(bytes)) => {
                seen.get() && bytes.len() == expected.len()
            }
            _ => false,
        }
    }
    fn begin_next(&mut self, next: Next) {
        match (self, next) {
            (Self::Source(reference, sent), Next::Source(value)) => {
                *reference = value;
                *sent = false;
            }
            (Self::Linear(back), Next::Linear) => {
                <FixedLinearBack<80, 1> as ReusableStepBack<PORTS>>::begin_next(back, ())
            }
            (Self::Sink(seen, expected), Next::Sink(bytes)) => {
                seen.set(false);
                *expected = bytes;
            }
            _ => unreachable!("all driver preflights passed before rearm"),
        }
    }
}
