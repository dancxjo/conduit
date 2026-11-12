//! Allocation-free deadline observation; no timer is armed or waited upon.
use super::{codec::ClockDisposition, owner::MonotonicDeadlineProvider};

/// The Host supplies calibrated milliseconds relative to this Boot's clock.
/// No epoch conversion, fallback time, identity or authority is inferred here.
pub type ReadBootMillis = fn() -> Result<u64, ClockDisposition>;

pub struct PollingClock {
    read: ReadBootMillis,
    last: Option<u64>,
    revoked: bool,
}

impl PollingClock {
    /// The caller supplies the actual calibrated boot-relative clock reading.
    /// This only constructs a reader; it creates no Ready Base or grant. The
    /// callback must perform one bounded read without allocating or waiting.
    pub const fn new(read: ReadBootMillis) -> Self {
        Self {
            read,
            last: None,
            revoked: false,
        }
    }

    fn sample(&mut self) -> Result<u64, ClockDisposition> {
        if self.revoked {
            return Err(ClockDisposition::ProviderLost);
        }
        let now = match (self.read)() {
            Ok(now) => now,
            Err(ClockDisposition::ProviderLost) => {
                self.revoked = true;
                return Err(ClockDisposition::ProviderLost);
            }
            Err(reason) => return Err(reason),
        };
        if self.last.is_some_and(|last| now < last) {
            self.revoked = true;
            return Err(ClockDisposition::ProviderLost);
        }
        self.last = Some(now);
        Ok(now)
    }
}

impl MonotonicDeadlineProvider for PollingClock {
    fn poll_until(&mut self, deadline_ms: u64) -> Result<Option<u64>, ClockDisposition> {
        let now = self.sample()?;
        Ok((now >= deadline_ms).then_some(now))
    }

    fn sample_now(&mut self) -> Result<u64, ClockDisposition> {
        self.sample()
    }

    fn revoke(&mut self) {
        // Revoke this loan alone. The callback has no stop/arm interface, so
        // neither a hardware clock nor an independent reader is affected.
        self.revoked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    std::thread_local! {
        static READING: Cell<Result<u64, ClockDisposition>> = const { Cell::new(Ok(0)) };
        static READS: Cell<usize> = const { Cell::new(0) };
    }

    fn read() -> Result<u64, ClockDisposition> {
        READS.with(|count| count.set(count.get() + 1));
        READING.with(Cell::get)
    }

    fn set(value: Result<u64, ClockDisposition>) {
        READING.with(|reading| reading.set(value));
    }

    fn count() -> usize {
        READS.with(Cell::get)
    }

    #[test]
    fn each_poll_or_sample_reads_once_and_preserves_actual_boot_milliseconds() {
        let mut clock = PollingClock::new(read);
        let before = count();
        set(Ok(0));
        assert_eq!(clock.poll_until(1), Ok(None));
        assert_eq!(count(), before + 1);
        set(Ok(1));
        assert_eq!(clock.poll_until(1), Ok(Some(1)));
        assert_eq!(count(), before + 2);
        set(Ok(37));
        assert_eq!(clock.poll_until(2), Ok(Some(37)));
        assert_eq!(clock.sample_now(), Ok(37));
        assert_eq!(count(), before + 4);
    }

    #[test]
    fn backwards_read_revokes_only_this_reader_without_further_reads() {
        let mut first = PollingClock::new(read);
        let mut other = PollingClock::new(read);
        set(Ok(8));
        assert_eq!(first.sample_now(), Ok(8));
        set(Ok(7));
        assert_eq!(first.poll_until(9), Err(ClockDisposition::ProviderLost));
        let before = count();
        set(Ok(10));
        assert_eq!(first.sample_now(), Err(ClockDisposition::ProviderLost));
        assert_eq!(first.poll_until(0), Err(ClockDisposition::ProviderLost));
        assert_eq!(count(), before);
        assert_eq!(other.sample_now(), Ok(10));
    }

    #[test]
    fn provider_loss_is_latched_and_other_refusals_are_preserved() {
        let mut clock = PollingClock::new(read);
        for reason in [
            ClockDisposition::Unavailable,
            ClockDisposition::UnsupportedDeadline,
            ClockDisposition::Malformed,
            ClockDisposition::Timeout,
        ] {
            set(Err(reason));
            let before = count();
            assert_eq!(clock.sample_now(), Err(reason));
            assert_eq!(count(), before + 1);
        }
        set(Ok(11));
        assert_eq!(clock.sample_now(), Ok(11));
        set(Err(ClockDisposition::ProviderLost));
        assert_eq!(clock.poll_until(12), Err(ClockDisposition::ProviderLost));
        let before = count();
        set(Ok(12));
        assert_eq!(clock.sample_now(), Err(ClockDisposition::ProviderLost));
        assert_eq!(count(), before);
    }

    #[test]
    fn explicit_revocation_does_not_read_or_stop_another_reader() {
        let mut first = PollingClock::new(read);
        let mut other = PollingClock::new(read);
        let before = count();
        first.revoke();
        first.revoke();
        assert_eq!(first.sample_now(), Err(ClockDisposition::ProviderLost));
        assert_eq!(first.poll_until(0), Err(ClockDisposition::ProviderLost));
        assert_eq!(count(), before);
        set(Ok(123));
        assert_eq!(other.poll_until(100), Ok(Some(123)));
        assert_eq!(count(), before + 1);
    }
}
