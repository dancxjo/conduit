//! Exact pre-trigger validation and resource reservation for std kernel runs.
//!
//! Migrated profiles use this boundary instead of instantiating the legacy
//! runtime merely to validate a plan and hold its resource pools.

use crate::installed_std::state_storage_profile;
use conduit_core::{
    resource_binding_satisfies, HostAdvertisement, PlanFragment, PlanId, ResourceBinding,
    ResourceClassId, ResourcePoolId, PROTOCOL_VERSION,
};
use conduit_plan_lowering::lowering::lower_plan_fragment_for_profile;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PoolUsage {
    pool_id: ResourcePoolId,
    class_id: ResourceClassId,
    capacity_units: u32,
    used_units: u32,
}

#[derive(Debug, Clone)]
pub(super) struct KernelResourceLedger {
    pools: Vec<PoolUsage>,
    instances: Vec<(conduit_core::CapabilityId, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KernelResourceReservation {
    plan_id: PlanId,
    bindings: Vec<ResourceBinding>,
    instances: Vec<(conduit_core::CapabilityId, u16)>,
}

impl KernelResourceLedger {
    pub(super) fn new(advertisement: &HostAdvertisement) -> Result<Self, String> {
        if advertisement.protocol_version != PROTOCOL_VERSION {
            return Err("kernel host advertisement uses the wrong protocol version".to_string());
        }
        let mut pools = Vec::with_capacity(advertisement.resources.len());
        for offer in &advertisement.resources {
            if offer.pool_id.as_str().is_empty()
                || offer.class_id.as_str().is_empty()
                || offer.capacity_units == 0
                || pools
                    .iter()
                    .any(|pool: &PoolUsage| pool.pool_id == offer.pool_id)
            {
                return Err("kernel host resource offers are malformed".to_string());
            }
            pools.push(PoolUsage {
                pool_id: offer.pool_id.clone(),
                class_id: offer.class_id.clone(),
                capacity_units: offer.capacity_units,
                used_units: 0,
            });
        }
        let mut instances = Vec::with_capacity(advertisement.capabilities.len());
        for capability in &advertisement.capabilities {
            if instances
                .iter()
                .any(|(id, _)| id == &capability.capability_id)
            {
                return Err("kernel host capability identities are duplicated".into());
            }
            instances.push((capability.capability_id.clone(), 0));
        }
        Ok(Self { pools, instances })
    }

    pub(super) fn prepare_and_reserve(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
    ) -> Result<KernelResourceReservation, String> {
        self.prepare_and_reserve_with_continuity(advertisement, fragment, false)
    }

    pub(super) fn prepare_and_reserve_with_continuity(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        continuity: bool,
    ) -> Result<KernelResourceReservation, String> {
        self.prepare_and_reserve_partitions(advertisement, &[(fragment, continuity)])?
            .pop()
            .ok_or_else(|| "single partition reservation disappeared".to_string())
    }

    /// Admit the complete local workload before committing any pool usage.
    /// Each reservation retains its original Plan identity. BodyPlan/workset
    /// validation remains the caller's responsibility; this grants no Play.
    pub(super) fn prepare_and_reserve_partitions(
        &mut self,
        advertisement: &HostAdvertisement,
        partitions: &[(&PlanFragment, bool)],
    ) -> Result<Vec<KernelResourceReservation>, String> {
        if partitions.is_empty() || partitions.len() > conduit_body::MAX_BODY_FORMS {
            return Err("local workload partition count exceeds the admitted profile".into());
        }
        for (index, (fragment, _)) in partitions.iter().enumerate() {
            if partitions[..index].iter().any(|(prior, _)| {
                prior.plan_id == fragment.plan_id && prior.fragment_id == fragment.fragment_id
            }) {
                return Err("duplicate local workload partition".into());
            }
        }
        // Staging is pre-Play, finite, and includes all existing reservations.
        // A late invalid partition or combined shortage discards the candidate
        // ledger without requiring fallible rollback of the live ledger.
        let mut staged = self.clone();
        let mut reservations = Vec::with_capacity(partitions.len());
        for (fragment, continuity) in partitions {
            reservations.push(staged.reserve_partition(advertisement, fragment, *continuity)?);
        }
        for (live, admitted) in self.pools.iter_mut().zip(staged.pools) {
            live.used_units = admitted.used_units;
        }
        for (live, admitted) in self.instances.iter_mut().zip(staged.instances) {
            live.1 = admitted.1;
        }
        Ok(reservations)
    }

    fn reserve_partition(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        continuity: bool,
    ) -> Result<KernelResourceReservation, String> {
        let mut profile = state_storage_profile();
        if continuity {
            profile = profile.with_owned_state_continuity();
        }
        let lowered = lower_plan_fragment_for_profile(fragment, profile)
            .map_err(|error| format!("kernel preparation lowering: {error:?}"))?;
        validate_exact_profile(advertisement, fragment)?;

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

        for pool in &self.pools {
            let requested = requested_units(&bindings, &pool.pool_id, &pool.class_id)?;
            let total = pool.used_units.checked_add(requested).ok_or_else(|| {
                format!("resource pool '{}' usage overflowed", pool.pool_id.as_str())
            })?;
            if total > pool.capacity_units {
                return Err(format!(
                    "resource pool '{}' requires {} units above capacity {}",
                    pool.pool_id.as_str(),
                    total,
                    pool.capacity_units
                ));
            }
        }
        for binding in &bindings {
            if !self
                .pools
                .iter()
                .any(|pool| pool.pool_id == binding.pool_id && pool.class_id == binding.class_id)
            {
                return Err(format!(
                    "resource binding '{}' is not offered by this host",
                    binding.pool_id.as_str()
                ));
            }
        }
        for pool in &mut self.pools {
            let requested = requested_units(&bindings, &pool.pool_id, &pool.class_id)?;
            pool.used_units += requested;
        }
        Ok(KernelResourceReservation {
            plan_id: fragment.plan_id.clone(),
            bindings,
            instances,
        })
    }

    pub(super) fn release(&mut self, reservation: KernelResourceReservation) -> Result<(), String> {
        for (id, released) in &reservation.instances {
            let (_, used) = self
                .instances
                .iter_mut()
                .find(|(candidate, _)| candidate == id)
                .ok_or_else(|| "released capability is absent from the ledger".to_string())?;
            *used = used
                .checked_sub(*released)
                .ok_or_else(|| "capability release exceeded its reservation".to_string())?;
        }
        for pool in &mut self.pools {
            let released = requested_units(&reservation.bindings, &pool.pool_id, &pool.class_id)?;
            pool.used_units = pool.used_units.checked_sub(released).ok_or_else(|| {
                format!(
                    "plan '{}' release exceeded resource pool '{}' reservation",
                    reservation.plan_id.as_str(),
                    pool.pool_id.as_str(),
                )
            })?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn allocation_capacity(&self) -> usize {
        self.pools.capacity()
    }
}

#[cfg(test)]
#[path = "kernel_partition_reservation_tests.rs"]
mod partition_tests;

fn validate_exact_profile(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
) -> Result<(), String> {
    if fragment.host_id != advertisement.host_id {
        return Err("fragment is assigned to a different host".to_string());
    }
    for placement in &fragment.placements {
        if placement.host_id != advertisement.host_id
            || placement.boot_id != advertisement.boot_id
            || placement.offer_generation != advertisement.offer_generation
        {
            return Err(format!(
                "placement '{}' does not target the current host boot and offer",
                placement.placement_id.as_str()
            ));
        }
        let capability = advertisement
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .ok_or_else(|| {
                format!(
                    "placement '{}' names an unavailable capability",
                    placement.placement_id.as_str()
                )
            })?;
        if let Some(binding) = placement.resources.iter().find(|binding| {
            !advertisement
                .resources
                .iter()
                .any(|offer| offer.pool_id == binding.pool_id)
        }) {
            return Err(format!(
                "resource pool '{}' is not offered by the current host",
                binding.pool_id.as_str()
            ));
        }
        let resources_match = capability.resource_requirements.len() == placement.resources.len()
            && capability.resource_requirements.iter().all(|requirement| {
                placement.resources.iter().any(|binding| {
                    advertisement
                        .resources
                        .iter()
                        .find(|offer| offer.pool_id == binding.pool_id)
                        .is_some_and(|offer| {
                            resource_binding_satisfies(binding, requirement, offer)
                        })
                })
            });
        let authority_match = capability.authority_requirements.len() == placement.authority.len()
            && capability.authority_requirements.iter().all(|requirement| {
                placement.authority.iter().any(|binding| {
                    !binding.grant_id.as_str().is_empty()
                        && binding.contract_id == requirement.contract_id
                        && binding.host_operation_contract_id
                            == requirement.host_operation_contract_id
                        && binding.subject_kind == requirement.subject_kind
                        && binding.host_id == placement.host_id
                        && binding.boot_id == placement.boot_id
                        && binding.capability_id == placement.capability_id
                })
            });
        if capability.kind_id != placement.kind_id
            || capability.kind_contract_revision != placement.kind_contract_revision
            || capability.implementation.execution_profile_id != placement.execution_profile_id
            || capability.implementation.implementation_id != placement.implementation_id
            || capability.implementation.artifact_id != placement.artifact_id
            || capability.inputs != placement.inputs
            || capability.outputs != placement.outputs
            || capability.host_operations != placement.host_operations
            || !resources_match
            || !authority_match
        {
            return Err(format!(
                "placement '{}' differs from the installed exact capability",
                placement.placement_id.as_str()
            ));
        }
        let active_instances = fragment
            .placements
            .iter()
            .filter(|candidate| candidate.capability_id == placement.capability_id)
            .count();
        if active_instances > usize::from(capability.limits.max_active_instances) {
            return Err(format!(
                "capability '{}' active-instance limit exceeded",
                capability.capability_id.as_str()
            ));
        }
    }
    Ok(())
}

fn requested_units(
    bindings: &[ResourceBinding],
    pool_id: &ResourcePoolId,
    class_id: &ResourceClassId,
) -> Result<u32, String> {
    bindings
        .iter()
        .filter(|binding| &binding.pool_id == pool_id && &binding.class_id == class_id)
        .try_fold(0_u32, |total, binding: &ResourceBinding| {
            total
                .checked_add(binding.units)
                .ok_or_else(|| format!("resource pool '{}' usage overflowed", pool_id.as_str()))
        })
}

#[cfg(test)]
mod tests {
    use super::KernelResourceLedger;
    use crate::kernel_multivalue::{advertisement, plan_local, profile_catalog};
    use conduit_core::{
        seal_plan, BootId, FormIdentity, HostId, ImplementationId, OfferGeneration, ResourcePoolId,
    };
    use conduit_form::parse;

    #[test]
    fn exact_reservation_rejects_overlap_releases_and_does_not_grow() {
        let host = advertisement(
            HostId::from("resource-host"),
            BootId::from("resource-boot"),
            OfferGeneration(1),
        );
        let form = parse(
            include_str!("../../../proof/fixtures/forms/kernel-multivalue.conduit"),
            &profile_catalog(),
        )
        .expect("multi-value form parses");
        let plan = plan_local(&form, &host).expect("multi-value plan resolves");
        let fragment = &plan.fragments[0];
        let mut ledger = KernelResourceLedger::new(&host).expect("ledger installs");
        let capacity = ledger.allocation_capacity();

        let first = ledger
            .prepare_and_reserve(&host, fragment)
            .expect("first exact reservation succeeds");
        assert_eq!(first.plan_id, fragment.plan_id);
        assert_eq!(first.bindings.len(), 3);
        let overlap = ledger
            .prepare_and_reserve(&host, fragment)
            .expect_err("second reservation exceeds the selected capability instance limit");
        assert!(
            overlap.contains("combined active-instance limit"),
            "{overlap}"
        );
        ledger.release(first).expect("terminal release succeeds");
        let second = ledger
            .prepare_and_reserve(&host, fragment)
            .expect("released capacity can be reserved again");
        ledger.release(second).expect("second release succeeds");
        assert_eq!(ledger.allocation_capacity(), capacity);

        let identity = FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        };
        let mut wrong_implementation = fragment.clone();
        wrong_implementation.placements[0].implementation_id =
            ImplementationId::from("std/not-installed@1");
        let wrong_implementation = seal_plan(identity.clone(), vec![wrong_implementation]);
        let error = ledger
            .prepare_and_reserve(&host, &wrong_implementation.fragments[0])
            .expect_err("resealed implementation lie must fail before reservation");
        assert!(error.contains("installed exact capability"), "{error}");

        let mut wrong_pool = fragment.clone();
        wrong_pool.placements[0].resources[0].pool_id = ResourcePoolId::from("not/offered");
        let wrong_pool = seal_plan(identity, vec![wrong_pool]);
        let error = ledger
            .prepare_and_reserve(&host, &wrong_pool.fragments[0])
            .expect_err("resealed resource-pool lie must fail before reservation");
        assert!(error.contains("not offered"), "{error}");
    }
}
