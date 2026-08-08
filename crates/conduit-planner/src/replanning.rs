use crate::characteristics::{seal_characteristics, select_realization_with_characteristics};
use crate::observations::validate_resource_observations;
use crate::realization::{consume_selected_capacity, reject_unknown_operation_inputs};
use crate::requirements::{validate_hard_requirements, HardRealizationRequirements};
use crate::{
    plan_with_options, PlacementChoices, PlannerError, PlanningOptions, RealizationPolicy,
};
use conduit_core::{
    ConnectionProvider, HostAdvertisement, OperationId, Plan, PlanId, RealizationAdvertisement,
    ResourceObservation,
};
use conduit_form::CheckedForm;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationReplanOutcome {
    Unchanged {
        plan_id: PlanId,
    },
    Replacement {
        previous_plan_id: PlanId,
        plan: Plan,
    },
}

/// Re-run ordinary realization planning against refreshed observations.
///
/// The previous Plan is borrowed and never rewritten. A changed realization
/// is returned only as a separately sealed replacement Plan.
#[allow(clippy::too_many_arguments)]
pub fn replan_selected_realizations_with_characteristics(
    previous: &Plan,
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    providers: &[ConnectionProvider],
    requirements: &BTreeMap<OperationId, HardRealizationRequirements>,
    advertisements: &[RealizationAdvertisement],
    observations: &[ResourceObservation],
    policies: &BTreeMap<OperationId, RealizationPolicy>,
    planning_options: PlanningOptions<'_>,
) -> Result<RealizationReplanOutcome, PlannerError> {
    if previous.source_document_id != form.source_document_id
        || previous.checked_form_id != form.checked_form_id
        || previous.expanded_form_id != form.expanded_form_id
    {
        return Err(PlannerError::InvalidFormIdentity(
            "previous Plan and replanning form identities differ".to_string(),
        ));
    }
    reject_unknown_operation_inputs(form, requirements, policies)?;
    validate_resource_observations(realm, observations)?;
    let mut remaining = observations.to_vec();
    let mut by_operation = BTreeMap::new();
    for operation in &form.operations {
        let choice = select_realization_with_characteristics(
            operation,
            realm,
            advertisements,
            requirements
                .get(&operation.operation_id)
                .unwrap_or(&HardRealizationRequirements::default()),
            &remaining,
            policies
                .get(&operation.operation_id)
                .unwrap_or(&RealizationPolicy::default()),
        )?;
        consume_selected_capacity(realm, &choice, &mut remaining)?;
        by_operation.insert(operation.operation_id.clone(), choice);
    }
    let placements = PlacementChoices { by_operation };
    let mut plain_requirements = requirements.clone();
    for requirement in plain_requirements.values_mut() {
        requirement.minimum_characteristic_counts.clear();
        requirement.maximum_characteristic_counts.clear();
        requirement.required_characteristic_flags.clear();
        requirement.required_characteristic_labels.clear();
    }
    validate_hard_requirements(form, realm, &placements, &plain_requirements)?;
    let replacement = seal_characteristics(
        plan_with_options(form, realm, &placements, providers, planning_options)?,
        advertisements,
    )?;
    if replacement.plan_id == previous.plan_id {
        Ok(RealizationReplanOutcome::Unchanged {
            plan_id: previous.plan_id.clone(),
        })
    } else {
        Ok(RealizationReplanOutcome::Replacement {
            previous_plan_id: previous.plan_id.clone(),
            plan: replacement,
        })
    }
}
