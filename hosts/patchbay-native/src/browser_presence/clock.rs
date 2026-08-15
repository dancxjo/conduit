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
        u64::try_from(self.origin.elapsed().as_millis())
            .map_err(|_| "browser presence clock overflowed".into())
    }

    pub(super) fn presentation_reference(&self) -> Result<TemporalReference, String> {
        Ok(self.reference_for_ticks(self.now_millis()?))
    }

    #[cfg(test)]
    fn reference_at_millis(&self, ticks: u64) -> TemporalReference {
        self.reference_for_ticks(ticks)
    }

    fn reference_for_ticks(&self, ticks: u64) -> TemporalReference {
        let scale = match self.descriptor.scale {
            HostPresenceClockScale::Milliseconds => TemporalScale::Milliseconds,
        };
        TemporalReference {
            identity: format!("reference/{}/{ticks}", self.descriptor.basis_id),
            instant: TemporalInstant {
                ticks,
                scale,
                clock_basis: self.descriptor.basis_id.clone(),
                resolution_ticks: self.descriptor.resolution_ticks,
                uncertainty_ticks: self.descriptor.uncertainty_ticks,
            },
        }
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
        let reference = first.presentation_reference().unwrap();
        assert_eq!(reference.instant.clock_basis, first.descriptor.basis_id);
        assert_eq!(reference.instant.scale, TemporalScale::Milliseconds);
        assert!(reference
            .identity
            .ends_with(&format!("/{}", reference.instant.ticks)));
    }

    #[test]
    fn exact_ticks_become_finite_same_basis_presentation_references() {
        let clock = BrowserPresenceClock::new().unwrap();
        for ticks in [0, 42, u64::MAX] {
            let reference = clock.reference_at_millis(ticks);
            assert_eq!(
                reference.identity,
                format!("reference/{}/{ticks}", clock.descriptor.basis_id)
            );
            assert!(reference.identity.len() <= conduit_presentation::MAX_PRESENTATION_ID_BYTES);
            assert_eq!(reference.instant.ticks, ticks);
            assert_eq!(reference.instant.scale, TemporalScale::Milliseconds);
            assert_eq!(reference.instant.clock_basis, clock.descriptor.basis_id);
            assert_eq!(
                reference.instant.resolution_ticks,
                clock.descriptor.resolution_ticks
            );
            assert_eq!(
                reference.instant.uncertainty_ticks,
                clock.descriptor.uncertainty_ticks
            );
            assert_eq!(clock.reference_at_millis(ticks), reference);
        }
    }

    #[test]
    fn restart_scoped_references_remain_incomparable() {
        let first = BrowserPresenceClock::new().unwrap();
        let second = BrowserPresenceClock::new().unwrap();
        let first_reference = first.reference_at_millis(42);
        let second_reference = second.reference_at_millis(42);
        assert_ne!(first_reference.identity, second_reference.identity);
        assert_eq!(
            first_reference
                .instant
                .relation_to(&second_reference.instant),
            Err(conduit_presentation::TemporalRelationError::Incomparable)
        );
    }
}
