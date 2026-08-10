use crate::observations::{observations_admit, validate_resource_observations};
use crate::policy::select_realization_matching;
use crate::prelude::*;
use crate::{
    plan_with_hard_requirements, HardRealizationRequirements, PlacementChoices, PlannerError,
    RealizationPolicy,
};
use alloc::collections::BTreeMap;
use conduit_core::{ConnectionBase, GearId, HostAdvertisement, Plan, ResourceObservation};
use conduit_form::CheckedForm;

/// Selects exact realizations for a whole checked form, sharing current finite
/// observed capacity across gears, then constructs the ordinary Plan.
pub fn plan_selected_realizations(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    bases: &[ConnectionBase],
    requirements: &BTreeMap<GearId, HardRealizationRequirements>,
    observations: &[ResourceObservation],
    policies: &BTreeMap<GearId, RealizationPolicy>,
) -> Result<Plan, PlannerError> {
    reject_unknown_operation_inputs(form, requirements, policies)?;
    validate_resource_observations(hosts, observations)?;
    let mut remaining = observations.to_vec();
    let mut placements = BTreeMap::new();

    for gear in &form.gears {
        let requirement = requirements.get(&gear.gear_id).cloned().unwrap_or_default();
        let policy = policies.get(&gear.gear_id).cloned().unwrap_or_default();
        let choice = select_realization_matching(
            gear,
            hosts,
            &requirement,
            &policy,
            |host, offer| observations_admit(host, offer, &remaining),
            Some(PlannerError::CurrentResourceObservationUnavailable(
                format!(
                    "gear '{}' has no realization within remaining observed capacity",
                    gear.gear_id.as_str()
                ),
            )),
        )?;
        consume_selected_capacity(hosts, &choice, &mut remaining)?;
        placements.insert(gear.gear_id.clone(), choice);
    }

    plan_with_hard_requirements(
        form,
        hosts,
        &PlacementChoices {
            by_gear: placements,
        },
        bases,
        requirements,
    )
}

pub(crate) fn reject_unknown_operation_inputs(
    form: &CheckedForm,
    requirements: &BTreeMap<GearId, HardRealizationRequirements>,
    policies: &BTreeMap<GearId, RealizationPolicy>,
) -> Result<(), PlannerError> {
    for gear_id in requirements.keys().chain(policies.keys()) {
        if !form.gears.iter().any(|gear| &gear.gear_id == gear_id) {
            return Err(PlannerError::UnknownGear(gear_id.as_str().to_string()));
        }
    }
    Ok(())
}

pub(crate) fn consume_selected_capacity(
    hosts: &[HostAdvertisement],
    choice: &crate::PlacementChoice,
    remaining: &mut [ResourceObservation],
) -> Result<(), PlannerError> {
    let host = hosts
        .iter()
        .find(|host| host.host_id == choice.host_id)
        .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().to_string()))?;
    let offer = host
        .capabilities
        .iter()
        .find(|offer| offer.capability_id == choice.capability_id)
        .ok_or_else(|| {
            PlannerError::UnknownCapability(choice.capability_id.as_str().to_string())
        })?;

    for requirement in &offer.resource_requirements {
        let observation = remaining
            .iter_mut()
            .find(|observation| {
                observation.host_id == host.host_id
                    && observation.boot_id == host.boot_id
                    && observation.offer_generation == host.offer_generation
                    && observation.class_id == requirement.class_id
                    && host.resources.iter().any(|pool| {
                        pool.pool_id == observation.pool_id
                            && pool.class_id == requirement.class_id
                            && if requirement.compute.is_some() {
                                conduit_core::compute_reservation(
                                    requirement,
                                    pool,
                                    observation.unreserved_units,
                                )
                                .is_some()
                            } else {
                                observation.unreserved_units >= requirement.units
                            }
                    })
            })
            .ok_or_else(|| {
                PlannerError::CurrentResourceObservationUnavailable(
                    "selected realization lost observed capacity".to_string(),
                )
            })?;
        // Candidate selection reserves only the admitted minimum. The Plan
        // builder distributes capacity toward preferences after all selected
        // gears' minima are known.
        observation.unreserved_units -= requirement.units;
    }
    Ok(())
}
