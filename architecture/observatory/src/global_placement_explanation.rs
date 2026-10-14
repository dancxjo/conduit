//! Read-only projection of planner-minted same-input comparison custody.
use crate::{
    cost_explanation::{exact, BoundedText},
    CostExplanationRefusal,
};
use alloc::string::String;
use conduit_core::{verify_plan, Plan};
use conduit_planner::resource_cost::*;
use core::fmt::{self, Write};

pub fn render_global_placement_comparison(
    comparison: &GlobalPlacementComparison<'_, '_>,
) -> Result<String, CostExplanationRefusal> {
    let mut out = BoundedText(String::new());
    render(comparison, &mut out).map_err(|_| CostExplanationRefusal::Extent)?;
    Ok(out.0)
}
fn score(label: &str, value: &GlobalPlacementScore, out: &mut impl Write) -> fmt::Result {
    write!(
        out,
        "{label} unknown_operations={} unknown_transfers={} known_upper_seconds=",
        value.unknown_operations, value.unknown_transfers
    )?;
    exact(value.known_upper_seconds, out)?;
    write!(
        out,
        " (known subtotal; excludes unknown operands) critical_path_upper_seconds="
    )?;
    if let Some(value) = value.critical_path_upper_seconds {
        exact(value, out)?;
    } else {
        write!(out, "unknown-or-not-selected")?;
    }
    writeln!(out)
}
fn plan(
    label: &str,
    plan: &Plan,
    comparison: &GlobalPlacementComparison<'_, '_>,
    out: &mut impl Write,
) -> fmt::Result {
    if !verify_plan(plan)
        || plan.source_document_id != comparison.plot().source_document_id
        || plan.checked_plot_id != comparison.plot().checked_plot_id
        || plan.expanded_plot_id != comparison.plot().expanded_plot_id
    {
        return Err(fmt::Error);
    }
    writeln!(
        out,
        "{label} plan={} source={} checked={} expanded={} fragments={}",
        plan.plan_id.as_str(),
        plan.source_document_id.as_str(),
        plan.checked_plot_id.as_str(),
        plan.expanded_plot_id.as_str(),
        plan.fragments.len()
    )?;
    for fragment in &plan.fragments {
        writeln!(out,"{label} fragment host={} boot={} generation={} selected_bindings={:?} admitted_connections={:?} shared_pools={:?}",fragment.host_id.as_str(),fragment.boot_id.as_str(),fragment.offer_generation.0,fragment.placements,fragment.connections,fragment.shared_pools)?;
    }
    Ok(())
}
fn render(c: &GlobalPlacementComparison<'_, '_>, out: &mut impl Write) -> fmt::Result {
    let inputs = c.inputs();
    writeln!(
        out,
        "global placement comparison: historical estimates, read-only, no resource grant"
    )?;
    writeln!(out,"same immutable admitted inputs; objective={:?} incremental_transfer_model={} maximum_states={}",c.objective(),c.includes_transfer(),c.budget().maximum_states)?;
    writeln!(out,"hard facts: hosts={} operations={} current_resource_observations={} advertisements={} bases={} authority_grants={} protected_resource_grants={} line_offers={} line_candidates={}",inputs.hosts.len(),inputs.operations.len(),inputs.observations.len(),inputs.advertisements.len(),inputs.bases.len(),inputs.options.authority_grants.len(),inputs.options.protected_resource_grants.len(),inputs.options.line_offers.len(),inputs.options.line_candidates.len())?;
    for observation in inputs.observations {
        writeln!(
            out,
            "original resource fact={:?} observation_window={:?}",
            observation.observation(),
            observation.window()
        )?;
    }
    if let Some(transfer) = c.transfer_inputs() {
        if transfer.forecasts.len() > conduit_planner::MAXIMUM_REALIZATION_DECISION_RECORDS {
            return Err(fmt::Error);
        }
        for forecast in transfer.forecasts {
            writeln!(out,"submitted incremental transfer sign={} source_host={} source_boot={} sink_host={} sink_boot={} extent={:?} window={:?} calibration={:?} route={:?} duration={:?}; historical evidence, not revalidated by UI",forecast.sign().as_str(),forecast.source().host_id.as_str(),forecast.source().boot_id.as_str(),forecast.sink().host_id.as_str(),forecast.sink().boot_id.as_str(),forecast.extent(),forecast.submitted_window(),forecast.submitted_calibration(),forecast.connection(),forecast.submitted_duration())?;
        }
    }
    match c.baseline() {
        Ok(baseline) => {
            plan("greedy", &baseline.plan, c, out)?;
            if let Some(metric) = c.baseline_score() {
                match metric {
                    Ok(metric) => score("greedy", metric, out)?,
                    Err(error) => writeln!(
                        out,
                        "greedy metric unavailable: {error:?}; no numeric comparison"
                    )?,
                }
            }
            let detail =
                crate::render_cost_planned_realizations(baseline).map_err(|_| fmt::Error)?;
            writeln!(out, "{detail}")?;
        }
        Err(error) => writeln!(
            out,
            "greedy baseline refused: {error:?}; no baseline Plan or invented cost"
        )?,
    }
    match c.global() {
        Ok(global) => {
            plan("global", &global.plan, c, out)?;
            score("global", &global.score, out)?;
            writeln!(out,"search disposition={:?} states={} complete_assignments={} refused_assignments={} capacity_pruned_prefixes={} admitted_candidates={}",global.disposition,global.states,global.complete_assignments,global.refused_assignments,global.capacity_pruned_prefixes,global.candidate_count)?;
            let status=match global.disposition {
                GlobalPlacementDisposition::Exhaustive if global.score.unknown_operations==0 && global.score.unknown_transfers==0 =>"optimal for the finite admitted cost model; estimates are not execution guarantees",
                GlobalPlacementDisposition::Exhaustive=>"exhaustive candidate search with unknown cost operands; complete objective unavailable",
                GlobalPlacementDisposition::StateLimit=>"search state budget exhausted; best-known feasible assignment, optimality unproven",
                GlobalPlacementDisposition::NumericLimit=>"numeric work budget exhausted; best-known feasible assignment, optimality unproven",
            };
            writeln!(out, "status: {status}")?;
        }
        Err(error) => writeln!(
            out,
            "global placement refused: {error:?}; no feasible executable Plan claimed"
        )?,
    }
    Ok(())
}
