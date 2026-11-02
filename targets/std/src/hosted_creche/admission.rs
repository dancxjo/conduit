//! Exact prepared Host/Body/Back/resource custody, before platform effects.
use super::*;
use conduit_body_make::deployment_runtime::CRECHE_KIND;
use conduit_body_make::deployment_runtime_catalog::{
    creche_std_offer, CRECHE_HOST_CALL, CRECHE_IMPLEMENTATION, CRECHE_RESOURCE,
};

pub(super) fn context(
    host: &HostAdvertisement,
    plan: &BodyPlan,
    play: &BodyPlayIdentity,
    resource: ResourceOffer,
    limits: CrecheLimits,
) -> Result<CrecheExecutionContext, String> {
    if !limits.valid() || plan.verify_seal().is_err() || !play.validate_for(plan) {
        return Err(
            "Crèche preparation requires finite limits and exact sealed Body Plan/Play".into(),
        );
    }
    if resource.class_id.as_str() != CRECHE_RESOURCE
        || resource.capacity_units != 1
        || resource.pool_id.as_str().is_empty()
        || resource.compute.is_some()
        || resource.content.is_some()
        || host
            .resources
            .iter()
            .filter(|r| r.pool_id == resource.pool_id)
            .count()
            != 1
        || !host.resources.contains(&resource)
    {
        return Err("Crèche resource is not one exact admitted Host encounter slot".into());
    }
    let expected = creche_std_offer();
    if host
        .capabilities
        .iter()
        .filter(|offer| offer.capability_id == expected.capability_id)
        .count()
        != 1
        || !host.capabilities.contains(&expected)
    {
        return Err("Crèche capability differs from its exact installed offer".into());
    }
    let mut selected = None;
    for partition in &plan.plots {
        for fragment in &partition.plan.fragments {
            for placement in &fragment.placements {
                let is_creche = placement.kind_id.as_str() == CRECHE_KIND
                    || placement.implementation_id.as_str() == CRECHE_IMPLEMENTATION
                    || placement
                        .host_calls
                        .iter()
                        .any(|c| c.contract_id.as_str() == CRECHE_HOST_CALL);
                if is_creche {
                    if selected.is_some() {
                        return Err("Crèche owner requires one exact operation in the Body".into());
                    }
                    if fragment.host_id != host.host_id
                        || fragment.boot_id != host.boot_id
                        || fragment.offer_generation != host.offer_generation
                    {
                        return Err("Crèche fragment names a stale or different Host".into());
                    }
                    validate_placement(placement, host, &expected, &resource)?;
                    selected = Some((
                        partition.plot.clone(),
                        fragment.fragment_id.clone(),
                        placement.clone(),
                    ));
                } else if placement
                    .resources
                    .iter()
                    .any(|r| r.pool_id == resource.pool_id)
                    && placement.host_id == host.host_id
                {
                    return Err("another operation consumes the admitted Crèche slot".into());
                }
            }
        }
    }
    // Mask machinery is not a second way to invoke this admitted encounter.
    if plan
        .mask_topologies
        .iter()
        .flat_map(|m| &m.chains)
        .flat_map(|c| &c.plan.fragments)
        .flat_map(|f| &f.placements)
        .any(|p| {
            p.kind_id.as_str() == CRECHE_KIND
                || p.implementation_id.as_str() == CRECHE_IMPLEMENTATION
                || p.host_calls
                    .iter()
                    .any(|c| c.contract_id.as_str() == CRECHE_HOST_CALL)
                || (p.host_id == host.host_id
                    && p.resources.iter().any(|r| r.pool_id == resource.pool_id))
        })
    {
        return Err("Crèche encounter cannot be owned by Mask realization machinery".into());
    }
    let (plot, fragment_id, placement) =
        selected.ok_or("Body Plan has no admitted Crèche operation")?;
    Ok(CrecheExecutionContext {
        host: host.clone(),
        plan: plan.clone(),
        play: play.clone(),
        plot,
        fragment_id,
        placement,
        resource,
        limits,
    })
}

fn validate_placement(
    p: &PlannedGear,
    host: &HostAdvertisement,
    offer: &conduit_core::CapabilityOffer,
    resource: &ResourceOffer,
) -> Result<(), String> {
    let binding = p.resources.first();
    if p.host_id != host.host_id
        || p.boot_id != host.boot_id
        || p.offer_generation != host.offer_generation
        || p.kind_id != offer.kind_id
        || p.kind_contract_revision != offer.kind_contract_revision
        || p.capability_id != offer.capability_id
        || p.implementation_id != offer.implementation.implementation_id
        || p.execution_profile_id != offer.implementation.execution_profile_id
        || p.artifact_id != offer.implementation.artifact_id
        || p.inputs != offer.inputs
        || p.outputs != offer.outputs
        || p.semantic_contract != offer.semantic_contract
        || p.limits != offer.limits
        || p.host_calls != offer.host_calls
        || !p.configuration.is_empty()
        || p.base.is_some()
        || !p.authority.is_empty()
        || !p.realization_characteristics.is_empty()
        || p.realization_properties != offer.realization_properties
        || !p.pool_references.is_empty()
        || !p.terminal_transductions.is_empty()
        || p.resources.len() != 1
        || binding.is_none_or(|b| {
            b.pool_id != resource.pool_id
                || b.class_id != resource.class_id
                || b.units != 1
                || b.protected.is_some()
                || b.compute.is_some()
                || b.content.is_some()
        })
    {
        return Err(
            "Crèche placement differs from the exact installed Back/resource contract".into(),
        );
    }
    Ok(())
}
