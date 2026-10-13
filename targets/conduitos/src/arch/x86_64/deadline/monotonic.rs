//! A physics-only monotonic provider backed by the existing calibrated counter.
use super::CandidateDeadline;
use crate::monotonic_clock::{codec::ClockDisposition, owner::MonotonicDeadlineProvider};

pub struct NativeMonotonicDeadlineClock {
    counter: CandidateDeadline,
    lifetime_millis: u32,
    last_observed: Option<u64>,
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
        last_observed: None,
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
    fn observed(&mut self, elapsed: Option<i64>) -> Result<u64, ClockDisposition> {
        let Some(now) = elapsed.and_then(|now| u64::try_from(now).ok()) else {
            self.revoked = true;
            return Err(ClockDisposition::ProviderLost);
        };
        if now >= u64::from(self.lifetime_millis)
            || self.last_observed.is_some_and(|last| now < last)
        {
            self.revoked = true;
            return Err(ClockDisposition::ProviderLost);
        }
        self.last_observed = Some(now);
        Ok(now)
    }
}
impl MonotonicDeadlineProvider for NativeMonotonicDeadlineClock {
    fn poll_until(&mut self, deadline_millis: u64) -> Result<Option<u64>, ClockDisposition> {
        self.check_request(deadline_millis)?;
        // Exactly one actual counter observation in each poll quantum.
        self.sample_now()
            .map(|now| (now >= deadline_millis).then_some(now))
    }
    fn sample_now(&mut self) -> Result<u64, ClockDisposition> {
        if self.revoked {
            return Err(ClockDisposition::ProviderLost);
        }
        self.observed(self.counter.elapsed_millis())
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
            last_observed: None,
            revoked: false,
        }
    }
    #[test]
    fn calibrated_observation_preserves_actual_time_and_finite_lifetime() {
        let mut clock = clock();
        assert_eq!(clock.observed(Some(9)), Ok(9));
        assert_eq!(clock.observed(Some(12)), Ok(12));
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
        assert_eq!(expired.observed(None), Err(ClockDisposition::ProviderLost));
        assert_eq!(expired.poll_until(0), Err(ClockDisposition::ProviderLost));
        let mut revoked = clock();
        revoked.revoke();
        assert_eq!(revoked.poll_until(0), Err(ClockDisposition::ProviderLost));
        let mut invalid = clock();
        assert_eq!(
            invalid.observed(Some(-1)),
            Err(ClockDisposition::ProviderLost)
        );
        assert_eq!(invalid.poll_until(0), Err(ClockDisposition::ProviderLost));
    }
    #[test]
    fn samples_share_deadline_basis_and_refuse_regression_or_expiry() {
        let mut clock = clock();
        assert_eq!(clock.observed(Some(20)), Ok(20));
        assert_eq!(
            clock.observed(Some(19)),
            Err(ClockDisposition::ProviderLost)
        );
        assert_eq!(clock.sample_now(), Err(ClockDisposition::ProviderLost));
        let mut expired = clock();
        assert_eq!(
            expired.observed(Some(100)),
            Err(ClockDisposition::ProviderLost)
        );
        assert_eq!(expired.sample_now(), Err(ClockDisposition::ProviderLost));
    }
}
