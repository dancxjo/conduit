//! A physics-only monotonic provider backed by the existing calibrated counter.
use super::CandidateDeadline;
use crate::monotonic_clock::{codec::ClockDisposition, owner::MonotonicDeadlineProvider};

pub struct NativeMonotonicDeadlineClock {
    counter: CandidateDeadline,
    lifetime_millis: u32,
    revoked: bool,
}

/// Retain a calibrated counter for one finite native provider lifetime.
///
/// # Safety
/// The native owner must retain the current Boot and counter realization for
/// this entire lifetime. HPET must have been initialized from the checked ACPI
/// mapping; replacement or teardown requires revocation first. This primitive
/// must be held by the admitted clock Host Call owner, not exposed to a plot.
/// The containing machine loop remains responsible for timer interrupts and
/// idling between bounded play steps; this provider never spins until a deadline.
pub unsafe fn admitted_monotonic_deadline_clock(
    lifetime_millis: u32,
) -> Result<NativeMonotonicDeadlineClock, ClockDisposition> {
    if lifetime_millis == 0 {
        return Err(ClockDisposition::UnsupportedDeadline);
    }
    let counter = CandidateDeadline::admit(lifetime_millis).ok_or(ClockDisposition::Unavailable)?;
    Ok(NativeMonotonicDeadlineClock {
        counter,
        lifetime_millis,
        revoked: false,
    })
}

impl NativeMonotonicDeadlineClock {
    fn check_request(&self, deadline_millis: u64) -> Result<(), ClockDisposition> {
        if self.revoked {
            return Err(ClockDisposition::ProviderLost);
        }
        // The counter's finite lifetime expires at this boundary; it cannot
        // truthfully promise completion at or beyond that instant.
        if deadline_millis >= u64::from(self.lifetime_millis) {
            return Err(ClockDisposition::UnsupportedDeadline);
        }
        Ok(())
    }
    fn observed(
        &mut self,
        deadline: u64,
        elapsed: Option<i64>,
    ) -> Result<Option<u64>, ClockDisposition> {
        let Some(now) = elapsed.and_then(|now| u64::try_from(now).ok()) else {
            self.revoked = true;
            return Err(ClockDisposition::ProviderLost);
        };
        Ok((now >= deadline).then_some(now))
    }
}
impl MonotonicDeadlineProvider for NativeMonotonicDeadlineClock {
    fn poll_until(&mut self, deadline_millis: u64) -> Result<Option<u64>, ClockDisposition> {
        self.check_request(deadline_millis)?;
        // Exactly one actual counter observation in each poll quantum.
        let elapsed = self.counter.elapsed_millis();
        self.observed(deadline_millis, elapsed)
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock() -> NativeMonotonicDeadlineClock {
        NativeMonotonicDeadlineClock {
            counter: CandidateDeadline::Tsc {
                start_ticks: 0,
                end_ticks: 100,
                ticks_per_millisecond: 1,
            },
            lifetime_millis: 100,
            revoked: false,
        }
    }
    #[test]
    fn calibrated_observation_preserves_actual_time_and_finite_lifetime() {
        let mut clock = clock();
        assert_eq!(clock.observed(10, Some(9)), Ok(None));
        assert_eq!(clock.observed(10, Some(12)), Ok(Some(12)));
        assert_eq!(clock.check_request(99), Ok(()));
        assert_eq!(
            clock.poll_until(100),
            Err(ClockDisposition::UnsupportedDeadline)
        );
        assert_eq!(
            clock.poll_until(u64::MAX),
            Err(ClockDisposition::UnsupportedDeadline)
        );
    }
    #[test]
    fn expiry_and_revocation_quarantine_the_provider_before_another_read() {
        let mut expired = clock();
        assert_eq!(
            expired.observed(10, None),
            Err(ClockDisposition::ProviderLost)
        );
        assert_eq!(expired.poll_until(0), Err(ClockDisposition::ProviderLost));
        let mut revoked = clock();
        revoked.revoke();
        assert_eq!(revoked.poll_until(0), Err(ClockDisposition::ProviderLost));
        let mut invalid = clock();
        assert_eq!(
            invalid.observed(0, Some(-1)),
            Err(ClockDisposition::ProviderLost)
        );
        assert_eq!(invalid.poll_until(0), Err(ClockDisposition::ProviderLost));
    }
}
