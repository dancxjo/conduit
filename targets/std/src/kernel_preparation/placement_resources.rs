//! Placement accounting shared by ordinary and explicitly prepared workers.
use super::*;
impl KernelResourceLedger {
    /// The caller verifies the sealed Plan and current selected offers before
    /// staging this mutation. This helper does not grant execution permission.
    pub(super) fn reserve_verified_placement_resources(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    ) -> Result<KernelResourceReservation, String> {
        let mut instances = Vec::new();
        for (id, used) in &mut self.instances {
            let requested = u16::try_from(
                fragment
                    .placements
                    .iter()
                    .filter(|placement| &placement.capability_id == id)
                    .count(),
            )
            .map_err(|_| "capability instance demand overflow".to_string())?;
            if requested == 0 {
                continue;
            }
            let offer = advertisement
                .capabilities
                .iter()
                .find(|offer| &offer.capability_id == id)
                .ok_or_else(|| "reserved capability is no longer offered".to_string())?;
            let total = used
                .checked_add(requested)
                .filter(|total| *total <= offer.limits.max_active_instances)
                .ok_or_else(|| {
                    format!(
                        "capability '{}' combined active-instance limit exceeded",
                        id.as_str()
                    )
                })?;
            *used = total;
            instances.push((id.clone(), requested));
        }
        if fragment.placements.iter().any(|placement| {
            !self
                .instances
                .iter()
                .any(|(id, _)| id == &placement.capability_id)
        }) {
            return Err("planned capability is absent from the initialized ledger".into());
        }

        if lowered.resources.len() != lowered.identity.resources.len() {
            return Err("lowered resource identity table width changed".to_string());
        }
        let mut bindings = Vec::with_capacity(lowered.resources.len());
        for (numeric, (node, resource, semantic)) in
            lowered.resources.iter().zip(&lowered.identity.resources)
        {
            if numeric.node != *node
                || numeric.binding.resource != *resource
                || numeric.binding.units != semantic.units
            {
                return Err("lowered numeric resource row lost its semantic identity".to_string());
            }
            bindings.push(semantic.clone());
        }

        PoolUsage::reserve_all(&mut self.pools, &bindings)?;
        Ok(KernelResourceReservation {
            plan_id: fragment.plan_id.clone(),
            bindings,
            instances,
        })
    }
}
