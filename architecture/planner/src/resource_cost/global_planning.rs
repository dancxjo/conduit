//! Bounded joint assignment through existing hard admission and Plan sealing.
//! Aggregate fulfillment Time is one explicit objective, not a Host rating.
use super::*;
use crate::{PlacementChoice, PlacementChoices, PlannerError, RealizationDecisionDisposition};
use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::{ExactNumericWork, ExactRational, Plan, ResourceObservation};
use conduit_plot::CheckedPlot;
mod objective;
mod prefix_capacity;
mod search;
mod transfer_objective;
pub use objective::{GlobalObjectiveRefusal, GlobalPlacementObjective};
use search::Search;
pub use transfer_objective::{CostCordTransferDemand, GlobalTransferInputs};

const MAXIMUM_GLOBAL_STATES: u32 = 65_536;
#[derive(Debug, Clone, Copy)]
pub struct GlobalPlacementBudget {
    pub maximum_states: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalPlacementDisposition {
    Exhaustive,
    StateLimit,
    NumericLimit,
}
/// Unknown operands are counted, never converted to numeric zero. The second
/// coordinate is explicitly only the subtotal of known upper fulfillment Time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalPlacementScore {
    pub unknown_operations: u16,
    pub unknown_transfers: u16,
    pub known_upper_seconds: ExactRational,
    /// None retains unknown operands, never a fabricated zero path duration.
    pub critical_path_upper_seconds: Option<ExactRational>,
}
#[derive(Debug)]
pub struct GlobalPlacementResult {
    pub plan: Plan,
    pub score: GlobalPlacementScore,
    pub disposition: GlobalPlacementDisposition,
    pub states: u32,
    pub complete_assignments: u32,
    pub refused_assignments: u32,
    /// Prefixes rejected because minimum reserved demand already exceeds capacity.
    pub capacity_pruned_prefixes: u32,
    pub candidate_count: usize,
    pub objective: GlobalPlacementObjective,
    pub includes_transfer: bool,
}
#[derive(Debug)]
pub enum GlobalPlacementRefusal {
    Input(CostEvidenceRefusal),
    Candidate(CostSelectionError),
    Objective(GlobalObjectiveRefusal),
    NoFeasibleAssignment {
        disposition: GlobalPlacementDisposition,
        states: u32,
        refused_assignments: u32,
        capacity_pruned_prefixes: u32,
    },
}
struct Candidate<'a> {
    choice: PlacementChoice,
    forecast: Option<&'a CostedForecast<'a>>,
}

/// Jointly enumerate already admitted candidates. A complete assignment becomes
/// eligible for comparison only after ordinary authority/resource/Line sealing
/// and revalidation against the original current observed capacity.
/// The reviewed objective here is lexicographic: fewer unknown operands, then
/// minimum sum of exact known upper fulfillment Time, then stable admitted-candidate enumeration order.
/// The default retains the aggregate fulfillment objective. The explicit
/// objective entrance can use DAG critical-path Time. Independently scoped
/// incremental Cord/Line transfer costs remain necessary before that model
/// can claim transport-sensitive end-to-end response Time.
pub fn plan_global_realizations_with_costs<'a>(
    plot: &CheckedPlot,
    inputs: CostPlanningInputs<'a, '_>,
    budget: GlobalPlacementBudget,
    work: &mut ExactNumericWork,
) -> Result<GlobalPlacementResult, GlobalPlacementRefusal> {
    plan_global_realizations_with_objective(
        plot,
        inputs,
        budget,
        GlobalPlacementObjective::AggregateFulfillmentTime,
        work,
    )
}

/// Choose the reviewed finite aggregate or DAG critical-path upper Time model.
/// Both are preparation objectives over scoped estimates, not deadline promises.
pub fn plan_global_realizations_with_objective<'a>(
    plot: &CheckedPlot,
    inputs: CostPlanningInputs<'a, '_>,
    budget: GlobalPlacementBudget,
    objective: GlobalPlacementObjective,
    work: &mut ExactNumericWork,
) -> Result<GlobalPlacementResult, GlobalPlacementRefusal> {
    plan_global(plot, inputs, budget, objective, None, work)
}

/// Critical-path Time with separately scoped incremental Cord transfer costs.
/// Unknown or stale local/remote edge estimates remain unknown. Operation
/// forecasts must declare zero transfer to prevent double counting.
pub fn plan_global_realizations_with_transfer_costs<'a>(
    plot: &CheckedPlot,
    inputs: CostPlanningInputs<'a, '_>,
    budget: GlobalPlacementBudget,
    transfer: GlobalTransferInputs<'a>,
    work: &mut ExactNumericWork,
) -> Result<GlobalPlacementResult, GlobalPlacementRefusal> {
    plan_global(
        plot,
        inputs,
        budget,
        GlobalPlacementObjective::CriticalPathFulfillmentTime,
        Some(transfer),
        work,
    )
}

fn plan_global<'a>(
    plot: &CheckedPlot,
    inputs: CostPlanningInputs<'a, '_>,
    budget: GlobalPlacementBudget,
    objective: GlobalPlacementObjective,
    transfer: Option<GlobalTransferInputs<'a>>,
    work: &mut ExactNumericWork,
) -> Result<GlobalPlacementResult, GlobalPlacementRefusal> {
    use GlobalPlacementRefusal as R;
    if plot.gears.is_empty()
        || plot.gears.len() > MAXIMUM_COST_PLANNING_OPERATIONS
        || plot.gears.len() != inputs.operations.len()
        || plot.connections.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS
        || plot.gears.iter().any(|gear| {
            gear.inputs.len() > MAXIMUM_COST_INPUTS || gear.outputs.len() > MAXIMUM_COST_INPUTS
        })
        || budget.maximum_states == 0
        || budget.maximum_states > MAXIMUM_GLOBAL_STATES
    {
        return Err(R::Input(CostEvidenceRefusal::InputLimit));
    }
    if let Some(transfer) = transfer {
        transfer_objective::validate_inputs(plot, transfer).map_err(R::Input)?;
    }
    let order = if objective == GlobalPlacementObjective::CriticalPathFulfillmentTime {
        Some(objective::topological_order(plot).map_err(R::Objective)?)
    } else {
        None
    };
    let mut grids = Vec::with_capacity(plot.gears.len());
    let mut plain_requirements = BTreeMap::new();
    for (gear, operation) in plot.gears.iter().zip(inputs.operations) {
        if operation.demand.class().gear() != gear {
            return Err(R::Input(CostEvidenceRefusal::WorkClassChanged));
        }
        let selection = select_realization_with_costs(
            operation.demand,
            CostSelectionInputs {
                hosts: inputs.hosts,
                contexts: inputs.contexts,
                advertisements: inputs.advertisements,
                requirements: operation.requirements,
                observations: inputs.observations,
                tie_policy: operation.tie_policy,
                forecasts: operation.forecasts,
                intent: operation.intent,
                unavailable: operation.unavailable,
            },
            work,
        )
        .map_err(R::Candidate)?;
        let ignore = operation.unavailable == UnavailableCostPolicy::IgnoreCostsIfIncomplete
            && selection
                .costs
                .iter()
                .any(|cost| !matches!(cost.availability, CostAvailability::Known(_)));
        let mut grid = Vec::with_capacity(selection.costs.len());
        for sign in &selection.realization.signs {
            if matches!(
                sign.disposition,
                RealizationDecisionDisposition::Rejected(_)
            ) {
                continue;
            }
            let forecast = selection
                .costs
                .iter()
                .find(|cost| {
                    *cost.host_id == sign.host_id && *cost.capability_id == sign.capability_id
                })
                .and_then(|cost| match cost.availability {
                    CostAvailability::Known(value) if !ignore => Some(value),
                    _ => None,
                });
            if operation.unavailable == UnavailableCostPolicy::RequireKnown && forecast.is_none() {
                continue;
            }
            grid.push(Candidate {
                choice: PlacementChoice {
                    host_id: sign.host_id.clone(),
                    capability_id: sign.capability_id.clone(),
                },
                forecast,
            });
        }
        grids.push(grid);
        let mut requirement = operation.requirements.clone();
        requirement.predicates.clear();
        requirement.minimum_characteristic_counts.clear();
        requirement.maximum_characteristic_counts.clear();
        requirement.required_characteristic_flags.clear();
        requirement.required_characteristic_labels.clear();
        plain_requirements.insert(gear.gear_id.clone(), requirement);
    }
    let candidate_count = grids.iter().map(Vec::len).sum();
    let mut search = Search {
        plot,
        inputs,
        grids: &grids,
        requirements: &plain_requirements,
        maximum_states: budget.maximum_states,
        states: 0,
        complete: 0,
        refused: 0,
        capacity_pruned: 0,
        disposition: GlobalPlacementDisposition::Exhaustive,
        best: None,
        objective,
        transfer,
        order: order.as_deref(),
        work,
    };
    search.visit(0, &mut Vec::with_capacity(plot.gears.len()));
    let Some((plan, score)) = search.best else {
        return Err(R::NoFeasibleAssignment {
            disposition: search.disposition,
            states: search.states,
            refused_assignments: search.refused,
            capacity_pruned_prefixes: search.capacity_pruned,
        });
    };
    Ok(GlobalPlacementResult {
        plan,
        score,
        disposition: search.disposition,
        states: search.states,
        complete_assignments: search.complete,
        refused_assignments: search.refused,
        capacity_pruned_prefixes: search.capacity_pruned,
        candidate_count,
        objective,
        includes_transfer: transfer.is_some(),
    })
}

fn observed_capacity(
    plan: &Plan,
    observations: &[TimedResourceObservation<'_>],
) -> Result<(), PlannerError> {
    let mut totals = BTreeMap::new();
    for placement in plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
    {
        for binding in &placement.resources {
            let key = (
                &placement.host_id,
                &placement.boot_id,
                placement.offer_generation,
                &binding.pool_id,
                &binding.class_id,
            );
            let used = totals.entry(key).or_insert(0u32);
            *used = used.checked_add(binding.units).ok_or_else(|| {
                PlannerError::CurrentResourceObservationUnavailable(
                    "joint reservation overflow".into(),
                )
            })?;
        }
    }
    for ((host, boot, generation, pool, class), units) in totals {
        let mut matching = observations
            .iter()
            .map(TimedResourceObservation::observation)
            .filter(|value: &&ResourceObservation| {
                &value.host_id == host
                    && &value.boot_id == boot
                    && value.offer_generation == generation
                    && &value.pool_id == pool
                    && &value.class_id == class
            });
        if !matching
            .next()
            .is_some_and(|value| value.unreserved_units >= units)
            || matching.next().is_some()
        {
            return Err(PlannerError::CurrentResourceObservationUnavailable(
                "joint reservation exceeds current observed capacity".into(),
            ));
        }
    }
    Ok(())
}
