//! Generic planning admission through attributable resource allowances.

use conduit_core::{
    resource_binding_satisfies, ConnectionBase, HostAdvertisement, ResourceAllowanceSet,
    ResourceHealth, ResourceObservation,
};
use conduit_form::CheckedForm;

use crate::{plan, PlacementChoices, PlannerError};

/// Plans normally, then refuses before returning a Plan unless every selected
/// binding satisfies its original requirement, allowance, unchanged Host
/// offer, and current observation. Allowance provenance remains opaque.
pub fn plan_with_resource_allowances(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[ConnectionBase],
    allowance_sets: &[ResourceAllowanceSet],
    observations: &[ResourceObservation],
) -> Result<conduit_core::Plan, PlannerError> {
    let plan = plan(form, hosts, placements, bases)?;
    for planned in plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
    {
        let host = hosts
            .iter()
            .find(|host| host.host_id == planned.host_id && host.boot_id == planned.boot_id)
            .ok_or_else(|| PlannerError::UnknownHost(planned.host_id.as_str().into()))?;
        let set = allowance_sets
            .iter()
            .find(|set| set.host_id == planned.host_id && set.boot_id == planned.boot_id)
            .ok_or_else(|| {
                PlannerError::ResourceAllowanceUnsatisfied("no resource allowance set".into())
            })?;
        let capability = host
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == planned.capability_id)
            .expect("ordinary planning selected an advertised capability");
        for requirement in &capability.resource_requirements {
            let binding = planned
                .resources
                .iter()
                .find(|binding| binding.class_id == requirement.class_id)
                .expect("ordinary planning bound every requirement");
            let observation = observations
                .iter()
                .find(|observation| {
                    observation.host_id == planned.host_id
                        && observation.boot_id == planned.boot_id
                        && observation.pool_id == binding.pool_id
                        && observation.class_id == binding.class_id
                })
                .ok_or_else(|| {
                    PlannerError::CurrentResourceObservationUnavailable(
                        binding.pool_id.as_str().into(),
                    )
                })?;
            validate(set, host, requirement, binding, observation)?;
        }
    }
    Ok(plan)
}

fn validate(
    set: &ResourceAllowanceSet,
    host: &HostAdvertisement,
    requirement: &conduit_core::ResourceRequirement,
    binding: &conduit_core::ResourceBinding,
    observation: &ResourceObservation,
) -> Result<(), PlannerError> {
    if set.offer_generation != host.offer_generation
        || observation.offer_generation != set.offer_generation
    {
        return Err(PlannerError::ResourceAllowanceUnsatisfied(
            "resource allowance Host epoch is stale".into(),
        ));
    }
    let allowance = set
        .allowances
        .iter()
        .find(|value| value.pool_id == binding.pool_id && value.class_id == binding.class_id)
        .ok_or_else(|| PlannerError::ResourceAllowanceUnsatisfied("pool is not allowed".into()))?;
    let offer = host
        .resources
        .iter()
        .find(|value| value.pool_id == binding.pool_id && value.class_id == binding.class_id)
        .ok_or_else(|| PlannerError::UnavailableResource(binding.pool_id.as_str().into()))?;
    if binding.units > allowance.maximum_units {
        return Err(PlannerError::ResourceAllowanceUnsatisfied(format!(
            "candidate uses {} units above admitted resource allowance {} from '{}'",
            binding.units,
            allowance.maximum_units,
            set.source_id.as_str()
        )));
    }
    if !resource_binding_satisfies(binding, requirement, offer) {
        return Err(PlannerError::ResourceAllowanceUnsatisfied(
            "candidate does not satisfy its original requirement".into(),
        ));
    }
    if observation.health != ResourceHealth::Ready || binding.units > observation.unreserved_units {
        return Err(PlannerError::CurrentResourceObservationUnavailable(
            binding.pool_id.as_str().into(),
        ));
    }
    Ok(())
}
