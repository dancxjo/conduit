//! Conservative temporary custody of the remaining authored Base memory pool.
//! This is accounting, not allocator containment or whole preparation-peak proof.
use super::{BaseBackingReservation, KernelResourceLedger};
use conduit_core::ResourceBinding;
#[cfg(all(target_os = "linux", feature = "linux-display"))]
use conduit_core::{BaseInstanceId, BaseRegistry, HostAdvertisement, ResourcePoolId};

impl KernelResourceLedger {
    #[cfg(all(target_os = "linux", feature = "linux-display"))]
    pub(crate) fn reserve_base_preparation_ceiling(
        &mut self,
        host: &HostAdvertisement,
        registry: &BaseRegistry,
        base: &BaseInstanceId,
        generation: u64,
        pool: &ResourcePoolId,
    ) -> Result<BaseBackingReservation, String> {
        let remaining = self
            .pools
            .iter()
            .find(|usage| &usage.pool_id == pool)
            .and_then(|usage| usage.capacity_units.checked_sub(usage.used_units))
            .filter(|bytes| *bytes != 0)
            .ok_or("declared Base preparation capacity is unavailable")?;
        self.reserve_base_backing(host, registry, base, generation, pool, remaining)
    }
    /// Call after actual prepared owners have been measured. The caller keeps
    /// this affine token until every charged owner is retired or transferred.
    pub(crate) fn retain_base_preparation_bytes(
        &mut self,
        token: &mut BaseBackingReservation,
        bytes: usize,
    ) -> Result<(), String> {
        self.require_available()?;
        let units = u32::try_from(bytes).map_err(|_| "prepared Base size exceeds pool units")?;
        if units == 0 || units > token.binding.units || !token.matches_ledger(self) {
            return Err("actual prepared Base owners exceed reserved preparation ceiling".into());
        }
        let released = token.binding.units - units;
        if released != 0 {
            let release = ResourceBinding {
                units: released,
                ..token.binding.clone()
            };
            super::pool_usage::PoolUsage::release_all(&mut self.pools, &[release])?;
            token.binding.units = units;
        }
        Ok(())
    }
}
