//! Typed cost selection feeding the ordinary whole-Plot Plan builder.
//! Preparation shares remaining observed capacity across Gears. This is the
//! deterministic greedy reference, not the global placement optimizer.
use super::*;
use crate::{
    HardRealizationRequirements, PerformanceIntent, PlacementChoices, PlannerError,
    PlanningOptions, RealizationPolicy,
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use conduit_core::{
    BaseImplementationId, ExactNumericWork, GearId, HostAdvertisement, Plan,
    RealizationAdvertisement,
};
use conduit_plot::CheckedPlot;

pub const MAXIMUM_COST_PLANNING_OPERATIONS: usize = 64;

/// One exact checked Gear's admitted finite demand and reviewed policy inputs.
/// Operations must cover the Plot once, in its checked Gear order.
pub struct CostPlanningOperation<'a> {
    pub demand: CostDemand<'a>,
    pub forecasts: &'a [CostedForecast<'a>],
    pub requirements: &'a HardRealizationRequirements,
    pub tie_policy: &'a RealizationPolicy,
    pub intent: PerformanceIntent,
    pub unavailable: UnavailableCostPolicy,
}

pub struct CostPlanningInputs<'a, 'selection> {
    pub hosts: &'a [HostAdvertisement],
    pub contexts: &'a [CostEvaluationContext<'a>],
    pub operations: &'a [CostPlanningOperation<'a>],
    pub advertisements: &'selection [RealizationAdvertisement],
    pub observations: &'selection [TimedResourceObservation<'selection>],
    pub bases: &'selection [BaseImplementationId],
    pub options: PlanningOptions<'selection>,
}

/// An ordinary immutable Plan plus the complete historical selection accounts.
/// Accounts borrow admitted producer evidence, never temporary capacity ledgers.
/// They remain estimates; the Plan does not turn them into resource authority.
#[derive(Debug)]
pub struct CostPlannedRealizations<'a> {
    pub plan: Plan,
    pub decisions: Vec<CostRealizationSelection<'a>>,
}

#[derive(Debug)]
pub enum CostPlanningError<'a> {
    Input(CostEvidenceRefusal),
    Selection {
        gear_id: GearId,
        error: Box<CostSelectionError>,
        preceding: Vec<CostRealizationSelection<'a>>,
    },
    Planning {
        error: Box<PlannerError>,
        decisions: Vec<CostRealizationSelection<'a>>,
    },
}

/// Select every checked Gear through the existing hard-before-cost evaluator,
/// account for finite resources, and use ordinary authority/Line/Plan sealing.
/// No partial executable Plan is returned on a later refusal. Earlier decisions
/// are retained as historical attempts, not committed resource occupations.
pub fn plan_selected_realizations_with_costs<'a>(
    plot: &CheckedPlot,
    inputs: CostPlanningInputs<'a, '_>,
    work: &mut ExactNumericWork,
) -> Result<CostPlannedRealizations<'a>, CostPlanningError<'a>> {
    if plot.gears.is_empty()
        || plot.gears.len() > MAXIMUM_COST_PLANNING_OPERATIONS
        || inputs.operations.len() != plot.gears.len()
        || inputs.hosts.len() > MAXIMUM_COST_CANDIDATES
        || inputs.contexts.len() > MAXIMUM_COST_CANDIDATES
        || inputs.observations.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS
        || inputs.advertisements.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS
    {
        return Err(CostPlanningError::Input(CostEvidenceRefusal::InputLimit));
    }
    plot.validate_identities()
        .map_err(|error| CostPlanningError::Planning {
            error: Box::new(PlannerError::InvalidPlotIdentity(
                alloc::string::ToString::to_string(&error),
            )),
            decisions: Vec::new(),
        })?;
    for (gear, operation) in plot.gears.iter().zip(inputs.operations) {
        if operation.demand.class().gear() != gear {
            return Err(CostPlanningError::Input(
                CostEvidenceRefusal::WorkClassChanged,
            ));
        }
    }
    let mut remaining = inputs
        .observations
        .iter()
        .map(|timed| timed.observation().clone())
        .collect::<Vec<_>>();
    crate::observations::validate_resource_observations(inputs.hosts, &remaining).map_err(
        |error| CostPlanningError::Planning {
            error: Box::new(error),
            decisions: Vec::new(),
        },
    )?;
    crate::characteristics::validate_advertisements(inputs.hosts, inputs.advertisements).map_err(
        |error| CostPlanningError::Planning {
            error: Box::new(error),
            decisions: Vec::new(),
        },
    )?;
    let mut decisions = Vec::with_capacity(plot.gears.len());
    let mut by_gear = BTreeMap::new();
    let mut requirements = BTreeMap::new();
    for (gear, operation) in plot.gears.iter().zip(inputs.operations) {
        // Preserve actual producer windows while accounting for preparation's
        // tentative reservations. This ledger is not a new producer sample.
        let timed = remaining
            .iter()
            .zip(inputs.observations)
            .map(|(observation, original)| {
                TimedResourceObservation::admit(observation, original.window().clone())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(CostPlanningError::Input)?;
        let decision = match select_realization_with_costs(
            operation.demand,
            CostSelectionInputs {
                hosts: inputs.hosts,
                contexts: inputs.contexts,
                advertisements: inputs.advertisements,
                requirements: operation.requirements,
                observations: &timed,
                tie_policy: operation.tie_policy,
                forecasts: operation.forecasts,
                intent: operation.intent,
                unavailable: operation.unavailable,
            },
            work,
        ) {
            Ok(decision) => decision,
            Err(error) => {
                return Err(CostPlanningError::Selection {
                    gear_id: gear.gear_id.clone(),
                    error: Box::new(error),
                    preceding: decisions,
                })
            }
        };
        let choice = decision.realization.choice.clone();
        decisions.push(decision);
        if let Err(error) =
            crate::realization::consume_selected_capacity(inputs.hosts, &choice, &mut remaining)
        {
            return Err(CostPlanningError::Planning {
                error: Box::new(error),
                decisions,
            });
        }
        by_gear.insert(gear.gear_id.clone(), choice);
        let mut plain = operation.requirements.clone();
        // Characteristics have already passed the generic evaluator. Follow
        // the existing characteristic Plan path's ordinary requirement check.
        plain.predicates.clear();
        plain.minimum_characteristic_counts.clear();
        plain.maximum_characteristic_counts.clear();
        plain.required_characteristic_flags.clear();
        plain.required_characteristic_labels.clear();
        requirements.insert(gear.gear_id.clone(), plain);
    }
    let placements = PlacementChoices { by_gear };
    let build = || {
        crate::requirements::validate_hard_requirements(
            plot,
            inputs.hosts,
            &placements,
            &requirements,
        )?;
        let plan = crate::plan_with_options(
            plot,
            inputs.hosts,
            &placements,
            inputs.bases,
            inputs.options,
        )?;
        crate::characteristic_sealing::seal_characteristics(plan, inputs.advertisements)
    };
    let plan = match build() {
        Ok(plan) => plan,
        Err(error) => {
            return Err(CostPlanningError::Planning {
                error: Box::new(error),
                decisions,
            })
        }
    };
    Ok(CostPlannedRealizations { plan, decisions })
}
