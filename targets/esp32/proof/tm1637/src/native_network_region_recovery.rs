//! Only authenticated affine Pending custody can erase network evidence.
use super::*;
use crate::native_restart::network_recovery::{RecoveryPermit, Storage as RecoveryStorage};
impl RecoveryStorage for NetworkRegion {
    fn identity(&self) -> [u8; 32] {
        NetworkRegion::identity(self)
    }
    fn capacity(&self) -> u32 {
        self.0.length
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), &'static str> {
        Storage::read(&mut self.0, at, out).map_err(|_| "network-recovery-read")
    }
    fn erase(&mut self, permit: &RecoveryPermit, sector: u8) -> Result<(), &'static str> {
        self.erase_recovery_sector(permit, sector)
    }
}
impl NetworkRegion {
    /// Closed-epoch authority is mandatory even for the live timed adapter.
    pub(crate) fn erase_recovery_sector(
        &self,
        permit: &RecoveryPermit,
        sector: u8,
    ) -> Result<(), &'static str> {
        let relative = u32::from(sector) * 4096;
        if !permit.permits(self.identity(), self.0.length, relative) {
            return Err("network-recovery-erase-authority");
        }
        let start = self
            .0
            .bound(relative, 4096)
            .map_err(|_| "network-recovery-geometry")?;
        #[cfg(feature = "local-appliance")]
        let started = embassy_time::Instant::now();
        let result = self.0.base.lock(|base| {
            NorFlash::erase(&mut *base.borrow_mut(), start, start + 4096)
                .map_err(|_| "network-recovery-erase")
        });
        #[cfg(feature = "local-appliance")]
        measurement::erase(started, result.is_err());
        result
    }
}
