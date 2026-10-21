//! Region resource accounting preparation only; this token cannot start a kernel Play.
use super::{pool_usage::PoolUsage, KernelResourceLedger, KernelResourceReservation};
use conduit_plan_lowering::execution_regions::CheckedRegionResources;

#[allow(
    dead_code,
    reason = "region worker installation is a separate unfinished boundary"
)]
pub(crate) struct RegionResourceReservation(KernelResourceReservation);

impl KernelResourceLedger {
    /// Hold the checked region lane and worker-memory bindings in the existing concrete ledger.
    /// Placement resources, initialized workers and backend eligibility remain
    /// separate admission steps. The ordinary runner still refuses regions.
    #[allow(
        dead_code,
        reason = "region worker installation is a separate unfinished boundary"
    )]
    pub(crate) fn reserve_region_resources(
        &mut self,
        checked: &CheckedRegionResources<'_, '_>,
    ) -> Result<RegionResourceReservation, String> {
        let host = checked.host();
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
        PoolUsage::reserve_all(&mut self.pools, &bindings)?;
        Ok(RegionResourceReservation(KernelResourceReservation {
            plan_id: checked.plan_id().clone(),
            bindings,
            instances: Vec::new(),
        }))
    }

    #[allow(
        dead_code,
        reason = "region worker installation is a separate unfinished boundary"
    )]
    pub(crate) fn release_region_resources(
        &mut self,
        reservation: RegionResourceReservation,
    ) -> Result<(), String> {
        self.release(reservation.0)
    }
}
