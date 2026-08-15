//! Restart-scoped process-relative clock for native browser presence.

use conduit_body::{HostPresenceClock, HostPresenceClockScale};
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
}
