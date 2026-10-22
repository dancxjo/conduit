//! Region resource accounting preparation only; this token cannot start a kernel Play.
use super::{pool_usage::PoolUsage, KernelResourceLedger, KernelResourceReservation};
use conduit_core::{HostAdvertisement, ResourceObservation};
use conduit_plan_lowering::execution_regions::PreparedExecutionRegions;

pub(crate) struct RegionResourceReservation(KernelResourceReservation);

impl KernelResourceLedger {
    /// Atomically reserve selected placements, lanes and managed worker memory.
    /// The service prepares every executable instance before calling this; the
    /// reservation alone is not permission to start a worker Play.
    pub(crate) fn reserve_worker_resources(
        &mut self,
        host: &HostAdvertisement,
        prepared: &PreparedExecutionRegions<'_>,
        observations: &[ResourceObservation],
    ) -> Result<RegionResourceReservation, String> {
        super::exact_profile::validate_selected_placements(host, prepared.fragment())?;
        if prepared.fragment().placements.iter().any(|p| {
            host.capabilities
                .iter()
                .find(|offer| offer.capability_id == p.capability_id)
                .is_none_or(|offer| offer.limits != p.limits)
        }) {
            return Err("worker placement limits differ from the current offer".into());
        }
        let checked = prepared
            .check_resource_offers(host, observations)
            .map_err(|e| format!("worker resource admission: {e:?}"))?;
        if self.identity
            != (
                host.host_id.clone(),
                host.boot_id.clone(),
                host.offer_generation,
            )
        {
            return Err(
                "region resource offers do not target the initialized ledger boot and generation"
                    .into(),
            );
        }
        for binding in checked.bindings() {
            let offer = host
                .resources
                .iter()
                .find(|o| o.pool_id == binding.pool_id)
                .ok_or_else(|| "checked region resource offer disappeared".to_string())?;
            if !self.pools.iter().any(|p| p.matches_offer(offer)) {
                return Err(
                    "checked region resource offer differs from the initialized ledger".into(),
                );
            }
        }
        let bindings: Vec<_> = checked.bindings().iter().map(|b| (*b).clone()).collect();
        let mut staged = self.clone();
        let mut reservation = staged.reserve_verified_placement_resources(
            host,
            prepared.fragment(),
            prepared.lowered(),
        )?;
        PoolUsage::reserve_all(&mut staged.pools, &bindings)?;
        reservation.bindings.extend(bindings);
        for (live, admitted) in self.pools.iter_mut().zip(&staged.pools) {
            live.copy_usage_from(admitted);
        }
        for (live, admitted) in self.instances.iter_mut().zip(&staged.instances) {
            live.1 = admitted.1;
        }
        Ok(RegionResourceReservation(reservation))
    }

    pub(crate) fn release_region_resources(
        &mut self,
        reservation: RegionResourceReservation,
    ) -> Result<(), String> {
        self.release(reservation.0)
    }
}
