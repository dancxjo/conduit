//! Root's calibrated read-only BCM2835 counter provider; no protocol delays.
use crate::{
    arch,
    monotonic_clock::{codec::ClockDisposition, owner::MonotonicDeadlineProvider},
};
pub(super) struct Counter {
    live: bool,
    last: u64,
}
impl Counter {
    pub(super) const fn new() -> Self {
        Self {
            live: true,
            last: 0,
        }
    }
}
impl MonotonicDeadlineProvider for Counter {
    fn poll_until(&mut self, deadline: u64) -> Result<Option<u64>, ClockDisposition> {
        let now = self.sample_now()?;
        Ok((now >= deadline).then_some(now))
    }
    fn sample_now(&mut self) -> Result<u64, ClockDisposition> {
        if !self.live {
            return Err(ClockDisposition::ProviderLost);
        }
        let now = arch::read_counter() / 1000;
        if now < self.last {
            return Err(ClockDisposition::ProviderLost);
        }
        self.last = now;
        Ok(now)
    }
    fn revoke(&mut self) {
        self.live = false;
    }
}
