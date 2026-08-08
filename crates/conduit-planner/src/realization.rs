use crate::observations::{observations_admit, validate_resource_observations};
use crate::policy::select_realization_matching;
use crate::{
    plan_with_hard_requirements, HardRealizationRequirements, PlacementChoices, PlannerError,
    RealizationPolicy,
};
use conduit_core::{ConnectionProvider, HostAdvertisement, OperationId, Plan, ResourceObservation};
use conduit_form::CheckedForm;
use std::collections::BTreeMap;

/// Selects exact realizations for a whole checked form, sharing current finite
/// observed capacity across operations, then constructs the ordinary Plan.
pub fn plan_selected_realizations(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    providers: &[ConnectionProvider],
    requirements: &BTreeMap<OperationId, HardRealizationRequirements>,
    observations: &[ResourceObservation],
    policies: &BTreeMap<OperationId, RealizationPolicy>,
) -> Result<Plan, PlannerError> {
    reject_unknown_operation_inputs(form, requirements, policies)?;
    validate_resource_observations(realm, observations)?;
    let mut remaining = observations.to_vec();
    let mut placements = BTreeMap::new();

    for operation in &form.operations {
        let requirement = requirements
            .get(&operation.operation_id)
            .cloned()
            .unwrap_or_default();
        let policy = policies
            .get(&operation.operation_id)
            .cloned()
            .unwrap_or_default();
        let choice = select_realization_matching(
            operation,
            realm,
            &requirement,
            &policy,
            |host, offer| observations_admit(host, offer, &remaining),
            Some(PlannerError::CurrentResourceObservationUnavailable(
                format!(
                    "operation '{}' has no realization within remaining observed capacity",
                    operation.operation_id.as_str()
                ),
            )),
        )?;
        consume_selected_capacity(realm, &choice, &mut remaining)?;
        placements.insert(operation.operation_id.clone(), choice);
    }

    plan_with_hard_requirements(
        form,
        realm,
        &PlacementChoices {
            by_operation: placements,
        },
        providers,
        requirements,
    )
}

fn reject_unknown_operation_inputs(
    form: &CheckedForm,
    requirements: &BTreeMap<OperationId, HardRealizationRequirements>,
    policies: &BTreeMap<OperationId, RealizationPolicy>,
) -> Result<(), PlannerError> {
    for operation_id in requirements.keys().chain(policies.keys()) {
        if !form
            .operations
            .iter()
            .any(|operation| &operation.operation_id == operation_id)
        {
            return Err(PlannerError::UnknownOperation(
                operation_id.as_str().to_string(),
            ));
        }
    }
    Ok(())
}

fn consume_selected_capacity(
    realm: &[HostAdvertisement],
    choice: &crate::PlacementChoice,
    remaining: &mut [ResourceObservation],
) -> Result<(), PlannerError> {
    let host = realm
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
                    && observation.unreserved_units >= requirement.units
            })
            .ok_or_else(|| {
                PlannerError::CurrentResourceObservationUnavailable(
                    "selected realization lost observed capacity".to_string(),
                )
            })?;
        observation.unreserved_units -= requirement.units;
    }
    Ok(())
}
