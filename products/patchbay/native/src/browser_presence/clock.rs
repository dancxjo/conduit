//! Restart-scoped process-relative clock for native browser presence.

use conduit_body::{HostPresenceClock, HostPresenceClockScale};
use conduit_presentation::{TemporalInstant, TemporalReference, TemporalScale};
use std::io::Read;
use std::time::Instant;

pub(super) struct BrowserPresenceClock {
    origin: Instant,
    descriptor: HostPresenceClock,
}

impl BrowserPresenceClock {
    pub(super) fn new() -> Result<Self, String> {
        let mut basis_entropy = [0_u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut basis_entropy))
            .map_err(|error| format!("cannot obtain browser presence clock identity: {error}"))?;
        let basis_suffix = basis_entropy
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let descriptor = HostPresenceClock::new(
            format!("clock/patchbay-browser-presence/{basis_suffix}"),
            HostPresenceClockScale::Milliseconds,
            1,
            1,
        )
        .map_err(|error| format!("invalid browser presence clock: {error:?}"))?;
        Ok(Self {
            origin: Instant::now(),
            descriptor,
        })
    }

    pub(super) fn descriptor(&self) -> &HostPresenceClock {
        &self.descriptor
    }

    pub(super) fn now_millis(&self) -> Result<u64, String> {
        let elapsed = u64::try_from(self.origin.elapsed().as_millis())
            .map_err(|_| "browser presence clock overflowed")?;
        // Keep the declared uncertainty interval representable at process start.
        Ok(elapsed.max(self.descriptor.uncertainty_ticks))
    }

    pub(super) fn presentation_reference(&self) -> Result<TemporalReference, String> {
        Ok(TemporalReference {
            identity: "reference/native-parts-presentation".into(),
            instant: TemporalInstant {
                ticks: self.now_millis()?,
                scale: TemporalScale::Milliseconds,
                clock_basis: self.descriptor.basis_id.clone(),
                resolution_ticks: self.descriptor.resolution_ticks,
                uncertainty_ticks: self.descriptor.uncertainty_ticks,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_relative_basis_is_fresh_and_uncertainty_is_explicit() {
        let first = BrowserPresenceClock::new().unwrap();
        let second = BrowserPresenceClock::new().unwrap();
        assert_ne!(first.descriptor.basis_id, second.descriptor.basis_id);
        assert_eq!(first.descriptor.scale, HostPresenceClockScale::Milliseconds);
        assert_eq!(first.descriptor.resolution_ticks, 1);
        assert_eq!(first.descriptor.uncertainty_ticks, 1);
    }

    #[test]
    fn presentation_reference_reuses_the_exact_presence_clock_basis() {
        let clock = BrowserPresenceClock::new().unwrap();
        let reference = clock.presentation_reference().unwrap();

        assert_eq!(reference.instant.clock_basis, clock.descriptor.basis_id);
        assert_eq!(reference.instant.scale, TemporalScale::Milliseconds);
        assert_eq!(reference.instant.resolution_ticks, 1);
        assert_eq!(reference.instant.uncertainty_ticks, 1);
    }
}
