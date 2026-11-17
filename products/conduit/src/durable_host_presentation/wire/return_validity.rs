//! Wait for an authenticated challenge's actual local not-before instant.
//! No protocol data or authority is held here; only original time bounds enter.
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClockSample {
    pub unix_millis: u64,
    pub monotonic: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotBeforeRefusal {
    InvalidInterval,
    Expired,
    DeadlineExceeded,
    ClockUnavailable,
    #[allow(dead_code)]
    // Injected waiters may cancel; the blocking client sleeps within its deadline.
    Cancelled,
}

/// Returns a freshly sampled, genuinely valid local wall time. The original
/// monotonic I/O deadline never moves, including if the wall clock moves back.
/// The caller supplies the actual protocol TTL bound rather than a local copy.
pub(crate) fn wait_until_valid(
    issued_at_millis: u64,
    expires_at_millis: u64,
    maximum_ttl_millis: u64,
    original_deadline: Instant,
    mut sample: impl FnMut() -> Result<ClockSample, NotBeforeRefusal>,
    mut wait: impl FnMut(Duration) -> Result<(), NotBeforeRefusal>,
) -> Result<u64, NotBeforeRefusal> {
    expires_at_millis
        .checked_sub(issued_at_millis)
        .filter(|duration| *duration > 0 && *duration <= maximum_ttl_millis)
        .ok_or(NotBeforeRefusal::InvalidInterval)?;
    loop {
        let now = sample()?;
        if now.unix_millis >= expires_at_millis {
            return Err(NotBeforeRefusal::Expired);
        }
        let remaining = original_deadline
            .checked_duration_since(now.monotonic)
            .filter(|remaining| !remaining.is_zero())
            .ok_or(NotBeforeRefusal::DeadlineExceeded)?;
        if now.unix_millis >= issued_at_millis {
            return Ok(now.unix_millis);
        }
        let until_issued = Duration::from_millis(issued_at_millis - now.unix_millis);
        // Arriving exactly at the deadline cannot leave a valid exchange.
        if until_issued >= remaining {
            return Err(NotBeforeRefusal::DeadlineExceeded);
        }
        wait(until_issued)?;
    }
}

#[cfg(test)]
#[path = "return_validity_tests.rs"]
mod tests;
