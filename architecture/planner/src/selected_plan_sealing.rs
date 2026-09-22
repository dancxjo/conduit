use crate::PlannerError;
use conduit_core::{HostAdvertisement, Plan, RealizationAdvertisement, ResourceObservation};

pub fn seal_exact_plan_with_selected_realizations(
    plan: Plan,
    hosts: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    observations: &[ResourceObservation],
) -> Result<Plan, PlannerError> {
    crate::observations::validate_resource_observations(hosts, observations)?;
    crate::characteristics::validate_advertisements(hosts, advertisements)?;
    for advertisement in advertisements {
        let placement = plan
            .fragments
            .iter()
            .flat_map(|fragment| fragment.placements.iter())
            .find(|placement| {
                placement.host_id == advertisement.host_id
                    && placement.boot_id == advertisement.boot_id
                    && placement.offer_generation == advertisement.offer_generation
                    && placement.capability_id == advertisement.capability_id
            })
            .ok_or_else(|| {
                PlannerError::InvalidHardRealizationRequirement(
                    "selected realization does not belong to the exact plan".into(),
                )
            })?;
        for resource in &placement.resources {
            if !observations.iter().any(|observation| {
                observation.host_id == placement.host_id
                    && observation.boot_id == placement.boot_id
                    && observation.offer_generation == placement.offer_generation
                    && observation.pool_id == resource.pool_id
            }) {
                return Err(PlannerError::CurrentResourceObservationUnavailable(
                    format!(
                        "gear '{}' selected resource '{}' without a current observation",
                        placement.gear_id.as_str(),
                        resource.pool_id.as_str()
                    ),
                ));
            }
        }
    }
    crate::characteristic_sealing::seal_characteristics(plan, advertisements)
}
