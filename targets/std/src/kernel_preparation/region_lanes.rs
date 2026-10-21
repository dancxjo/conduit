//! Lane accounting preparation only; this token cannot start a kernel Play.
use super::{pool_usage::PoolUsage, KernelResourceLedger, KernelResourceReservation};
use conduit_plan_lowering::execution_regions::CheckedRegionLaneOffers;

#[allow(
    dead_code,
    reason = "region worker installation is a separate unfinished boundary"
)]
pub(super) struct RegionLaneReservation(KernelResourceReservation);

impl KernelResourceLedger {
    /// Hold the checked region lane bindings in the existing concrete ledger.
    /// Placement resources, initialized workers and backend eligibility remain
    /// separate admission steps. The ordinary runner still refuses regions.
    #[allow(
        dead_code,
        reason = "region worker installation is a separate unfinished boundary"
    )]
    pub(super) fn reserve_region_lanes(
        &mut self,
        checked: &CheckedRegionLaneOffers<'_, '_>,
    ) -> Result<RegionLaneReservation, String> {
        let host = checked.host();
        if self.identity
            != (
                host.host_id.clone(),
                host.boot_id.clone(),
                host.offer_generation,
            )
        {
            return Err(
                "lane offers do not target the initialized ledger boot and generation".into(),
            );
        }
        for binding in checked.bindings() {
            let offer = host
                .resources
                .iter()
                .find(|o| o.pool_id == binding.pool_id)
                .ok_or_else(|| "checked lane offer disappeared".to_string())?;
            if !self.pools.iter().any(|p| p.matches_offer(offer)) {
                return Err("checked lane offer differs from the initialized ledger".into());
            }
        }
        let bindings: Vec<_> = checked.bindings().iter().map(|b| (*b).clone()).collect();
        PoolUsage::reserve_all(&mut self.pools, &bindings)?;
        Ok(RegionLaneReservation(KernelResourceReservation {
            plan_id: checked.plan_id().clone(),
            bindings,
            instances: Vec::new(),
        }))
    }

    #[allow(
        dead_code,
        reason = "region worker installation is a separate unfinished boundary"
    )]
    pub(super) fn release_region_lanes(
        &mut self,
        reservation: RegionLaneReservation,
    ) -> Result<(), String> {
        self.release(reservation.0)
    }
}
