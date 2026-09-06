//! Exact pre-trigger validation and resource reservation for std kernel runs.
//!
//! Migrated profiles use this boundary instead of instantiating the legacy
//! runtime merely to validate a plan and hold its resource pools.

use crate::installed_std::lower_fragment;
use conduit_core::{
    resource_binding_satisfies, HostAdvertisement, PlanFragment, PlanId, ResourceBinding,
    ResourceClassId, ResourcePoolId, PROTOCOL_VERSION,
};

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KernelResourceReservation {
    plan_id: PlanId,
    bindings: Vec<ResourceBinding>,
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
        Ok(Self { pools })
    }

    pub(super) fn prepare_and_reserve(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
    ) -> Result<KernelResourceReservation, String> {
        let lowered = lower_fragment(fragment)
            .map_err(|error| format!("kernel preparation lowering: {error:?}"))?;
        validate_exact_profile(advertisement, fragment)?;

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
        })
    }

    pub(super) fn release(&mut self, reservation: KernelResourceReservation) -> Result<(), String> {
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
            .expect_err("second reservation exceeds exact pools");
        assert!(overlap.contains("above capacity"), "{overlap}");
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
