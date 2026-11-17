//! Concrete hosted monotonic timer and its exact prepared scalar basis.
use super::*;
pub struct ThreadTimer;

static THREAD_TIMER_EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
static THREAD_TIMER_BASIS: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

impl TimerAdapter for ThreadTimer {
    fn wait(&mut self, duration: Duration) {
        thread::sleep(duration);
    }

    fn monotonic_now_ms(&mut self) -> Option<u64> {
        u64::try_from(
            THREAD_TIMER_EPOCH
                .get_or_init(Instant::now)
                .elapsed()
                .as_millis(),
        )
        .ok()
    }

    fn monotonic_now_micros(&mut self) -> Option<u64> {
        u64::try_from(
            THREAD_TIMER_EPOCH
                .get_or_init(Instant::now)
                .elapsed()
                .as_micros(),
        )
        .ok()
    }

    fn monotonic_identity_backing_bound(
        &self,
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
    ) -> Option<usize> {
        host.0
            .len()
            .checked_add(boot.0.len())?
            .checked_add("std/thread-timer/process-epoch/".len() + 8 + 32)
    }

    fn monotonic_ticks_on(&mut self, basis: &conduit_core::MonotonicClockIdentity) -> Option<u64> {
        if basis.validate().is_err()
            || basis.scale() != conduit_core::TemporalScale::Microseconds
            || basis.resolution_ticks() != 1
            || basis.uncertainty_ticks() != 1
            || THREAD_TIMER_BASIS.get()?.as_deref()? != basis.basis_id()
        {
            return None;
        }
        self.monotonic_now_micros()
    }

    fn monotonic_observation(
        &mut self,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
    ) -> Option<conduit_core::MonotonicInstant> {
        let basis = THREAD_TIMER_BASIS
            .get_or_init(|| {
                let mut nonce = [0u8; 16];
                getrandom::fill(&mut nonce).ok()?;
                Some(format!(
                    "std/thread-timer/process-epoch/{:08x}{}",
                    std::process::id(),
                    nonce
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                ))
            })
            .as_ref()?;
        let identity = conduit_core::MonotonicClockIdentity::new(
            host_id.clone(),
            boot_id.clone(),
            basis.clone(),
            conduit_core::TemporalScale::Microseconds,
            1,
            1,
        )
        .ok()?;
        conduit_core::MonotonicInstant::new(self.monotonic_now_micros()?, identity).ok()
    }
}
