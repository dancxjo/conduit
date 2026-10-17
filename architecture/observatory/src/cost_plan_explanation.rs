//! Historical cost decisions correlated with the ordinary sealed Plan.
use crate::{
    render_cost_selection_explanation, render_cost_selection_refusal, CostExplanationRefusal,
    MAXIMUM_COST_EXPLANATION_BYTES,
};
use alloc::string::String;
use conduit_core::verify_plan;
use conduit_planner::{
    resource_cost::{CostPlannedRealizations, CostPlanningError, MAXIMUM_COST_PLANNING_OPERATIONS},
    RealizationDecisionDisposition,
};
use core::fmt::Write;

/// Preserve the exact Plan identity and every existing decision explanation.
/// This verifies custody for face; it does not re-admit resources or
/// turn an estimated cost into a grant. Combined output is bounded to 512 KiB.
pub fn render_cost_planned_realizations(
    planned: &CostPlannedRealizations<'_>,
) -> Result<String, CostExplanationRefusal> {
    use CostExplanationRefusal as R;
    if planned.decisions.is_empty() || planned.decisions.len() > MAXIMUM_COST_PLANNING_OPERATIONS {
        return Err(R::Extent);
    }
    if planned.plan.fragments.len() > MAXIMUM_COST_PLANNING_OPERATIONS
        || planned
            .plan
            .fragments
            .iter()
            .try_fold(0_usize, |count, fragment| {
                count.checked_add(fragment.placements.len())
            })
            .is_none_or(|count| count > MAXIMUM_COST_PLANNING_OPERATIONS)
    {
        return Err(R::Extent);
    }
    if !verify_plan(&planned.plan) {
        return Err(R::InconsistentSelection);
    }
    let placements: alloc::vec::Vec<_> = planned
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect();
    if placements.len() != planned.decisions.len() {
        return Err(R::InconsistentSelection);
    }
    let mut out = String::new();
    writeln!(
        out,
        "historical cost-selected Plan {}",
        planned.plan.plan_id.as_str()
    )
    .map_err(|_| R::Extent)?;
    out.push_str("capacity basis: captured current observations minus tentative prior Gear minimum reservations; this ledger is not a new producer sample or committed occupation\n");
    for (index, decision) in planned.decisions.iter().enumerate() {
        let mut selected = decision
            .realization
            .signs
            .iter()
            .filter(|record| record.disposition == RealizationDecisionDisposition::Selected);
        let record = selected.next().ok_or(R::InconsistentSelection)?;
        if selected.next().is_some()
            || planned.decisions[..index].iter().any(|previous| {
                previous.realization.signs.iter().any(|prior| {
                    prior.disposition == RealizationDecisionDisposition::Selected
                        && prior.gear_id == record.gear_id
                })
            })
        {
            return Err(R::InconsistentSelection);
        }
        let placement = placements
            .iter()
            .find(|placement| placement.gear_id == record.gear_id)
            .ok_or(R::InconsistentSelection)?;
        if placement.host_id != record.host_id
            || placement.boot_id != record.boot_id
            || placement.offer_generation != record.offer_generation
            || placement.capability_id != record.capability_id
            || placement.implementation_id != record.implementation_id
            || placement.artifact_id != record.artifact_id
        {
            return Err(R::InconsistentSelection);
        }
        let explanation = render_cost_selection_explanation(decision)?;
        if out
            .len()
            .checked_add(explanation.len())
            .is_none_or(|len| len > MAXIMUM_COST_EXPLANATION_BYTES)
        {
            return Err(R::Extent);
        }
        out.push_str(&explanation);
    }
    Ok(out)
}

/// Earlier per-Gear decisions are attempts; a later refusal produced no Plan.
/// Missing accounts refuse face rather than inventing cost evidence.
pub fn render_cost_planning_refusal(
    error: &CostPlanningError<'_>,
) -> Result<String, CostExplanationRefusal> {
    use CostExplanationRefusal as R;
    let (preceding, refused) = match error {
        CostPlanningError::Selection {
            preceding, error, ..
        } => (preceding, Some(render_cost_selection_refusal(error)?)),
        CostPlanningError::Planning { decisions, .. } if !decisions.is_empty() => (decisions, None),
        _ => return Err(R::NoRetainedDecision),
    };
    if preceding.len() > MAXIMUM_COST_PLANNING_OPERATIONS {
        return Err(R::Extent);
    }
    let mut out = String::from("historical cost planning refusal; no committed Plan was produced\nprior selections are preparation attempts, not occupied resources\n");
    for decision in preceding {
        append_bounded(&mut out, &render_cost_selection_explanation(decision)?)?;
    }
    if let Some(refused) = refused {
        append_bounded(&mut out, &refused)?;
    } else {
        append_bounded(
            &mut out,
            "ordinary Plan construction refused after these selections\n",
        )?;
    }
    Ok(out)
}

fn append_bounded(out: &mut String, text: &str) -> Result<(), CostExplanationRefusal> {
    if out
        .len()
        .checked_add(text.len())
        .is_none_or(|len| len > MAXIMUM_COST_EXPLANATION_BYTES)
    {
        return Err(CostExplanationRefusal::Extent);
    }
    out.push_str(text);
    Ok(())
}
