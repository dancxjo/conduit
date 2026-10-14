//! Private comparison custody over the same immutable admitted planning inputs.
use super::*;
use conduit_core::ExactNumericRefusal;

#[derive(Debug)]
pub enum GlobalBaselineMetricRefusal {
    Numeric(ExactNumericRefusal),
    Objective(GlobalObjectiveRefusal),
    Transfer(CostEvidenceRefusal),
    UnscopedTransfer,
    TransferSelection,
    MissingSelectedAccount,
}
/// Construction runs the existing planner entrances, never UI scheduling logic.
pub struct GlobalPlacementComparison<'a, 'selection> {
    plot: &'a CheckedPlot,
    inputs: CostPlanningInputs<'a, 'selection>,
    budget: GlobalPlacementBudget,
    objective: GlobalPlacementObjective,
    transfer: Option<GlobalTransferInputs<'a>>,
    baseline: Result<CostPlannedRealizations<'a>, CostPlanningError<'a>>,
    baseline_score: Option<Result<GlobalPlacementScore, GlobalBaselineMetricRefusal>>,
    global: Result<GlobalPlacementResult, GlobalPlacementRefusal>,
}
impl<'a, 'selection> GlobalPlacementComparison<'a, 'selection> {
    pub fn plot(&self) -> &CheckedPlot {
        self.plot
    }
    pub fn inputs(&self) -> CostPlanningInputs<'a, 'selection> {
        self.inputs
    }
    pub fn budget(&self) -> GlobalPlacementBudget {
        self.budget
    }
    pub fn objective(&self) -> GlobalPlacementObjective {
        self.objective
    }
    pub fn includes_transfer(&self) -> bool {
        self.transfer.is_some()
    }
    pub fn transfer_inputs(&self) -> Option<GlobalTransferInputs<'a>> {
        self.transfer
    }
    pub fn baseline(&self) -> &Result<CostPlannedRealizations<'a>, CostPlanningError<'a>> {
        &self.baseline
    }
    pub fn baseline_score(
        &self,
    ) -> Option<&Result<GlobalPlacementScore, GlobalBaselineMetricRefusal>> {
        self.baseline_score.as_ref()
    }
    pub fn global(&self) -> &Result<GlobalPlacementResult, GlobalPlacementRefusal> {
        &self.global
    }
}
#[allow(clippy::too_many_arguments)]
pub fn compare_global_placement<'a, 'selection>(
    plot: &'a CheckedPlot,
    inputs: CostPlanningInputs<'a, 'selection>,
    budget: GlobalPlacementBudget,
    objective: GlobalPlacementObjective,
    transfer: Option<GlobalTransferInputs<'a>>,
    baseline_work: &mut ExactNumericWork,
    global_work: &mut ExactNumericWork,
) -> GlobalPlacementComparison<'a, 'selection> {
    let baseline = plan_selected_realizations_with_costs(plot, inputs, baseline_work);
    let baseline_score = baseline
        .as_ref()
        .ok()
        .map(|baseline| score_baseline(plot, inputs, baseline, objective, transfer, baseline_work));
    let global = plan_global(plot, inputs, budget, objective, transfer, global_work);
    GlobalPlacementComparison {
        plot,
        inputs,
        budget,
        objective,
        transfer,
        baseline,
        baseline_score,
        global,
    }
}
fn score_baseline(
    plot: &CheckedPlot,
    inputs: CostPlanningInputs<'_, '_>,
    baseline: &CostPlannedRealizations<'_>,
    selected_objective: GlobalPlacementObjective,
    transfer: Option<GlobalTransferInputs<'_>>,
    work: &mut ExactNumericWork,
) -> Result<GlobalPlacementScore, GlobalBaselineMetricRefusal> {
    use GlobalBaselineMetricRefusal as R;
    let mut score = GlobalPlacementScore {
        unknown_operations: 0,
        unknown_transfers: 0,
        known_upper_seconds: ExactRational::ZERO,
        critical_path_upper_seconds: None,
    };
    let mut costs = Vec::with_capacity(baseline.decisions.len());
    for decision in &baseline.decisions {
        let account = decision
            .costs
            .iter()
            .find(|account| account.selected)
            .ok_or(R::MissingSelectedAccount)?;
        let value = match &account.availability {
            CostAvailability::Known(forecast) => {
                if transfer.is_some()
                    && !forecast.forecast().stages().transfer.is_some_and(|range| {
                        range.lower().seconds() == ExactRational::ZERO
                            && range.upper().seconds() == ExactRational::ZERO
                    })
                {
                    return Err(R::UnscopedTransfer);
                }
                let value = forecast.upper_seconds();
                score.known_upper_seconds = score
                    .known_upper_seconds
                    .checked_add(value, work)
                    .map_err(R::Numeric)?;
                Some(value)
            }
            _ => {
                score.unknown_operations += 1;
                None
            }
        };
        costs.push(value);
    }
    let edges = if let Some(transfer) = transfer {
        transfer_objective::validate_inputs(plot, transfer).map_err(R::Transfer)?;
        let edges = transfer_objective::edge_costs(plot, &baseline.plan, transfer, inputs)
            .map_err(|_| R::TransferSelection)?;
        score.unknown_transfers = edges.iter().filter(|e| e.is_none()).count() as u16;
        Some(edges)
    } else {
        None
    };
    if selected_objective == GlobalPlacementObjective::CriticalPathFulfillmentTime
        && score.unknown_operations == 0
        && score.unknown_transfers == 0
    {
        let order = objective::topological_order(plot).map_err(R::Objective)?;
        let costs: Vec<_> = costs
            .into_iter()
            .map(|c| c.expect("known operands established"))
            .collect();
        let edges = edges
            .map(|e| {
                e.into_iter()
                    .map(|v| v.expect("known operands established"))
                    .collect()
            })
            .unwrap_or_else(|| alloc::vec![ExactRational::ZERO;plot.connections.len()]);
        score.critical_path_upper_seconds = Some(
            objective::critical_path_with_transfers(plot, &order, &costs, &edges, work)
                .map_err(R::Numeric)?,
        );
    }
    Ok(score)
}
