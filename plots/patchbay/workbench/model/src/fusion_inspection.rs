//! Borrow historical planner decisions beside the exact inspected Plan.
use super::*;
use conduit_planner::{
    CandidatePlacementDisposition as Disposition, FusionSelection, OptimizedPlan,
    MAXIMUM_FUSION_CANDIDATES, MAXIMUM_FUSION_OFFERS,
};
use std::collections::BTreeSet;

impl PatchbayTopology {
    /// No decision history or resource authority is created here. The caller
    /// supplies the planner's retained result for one already inspected Plan.
    pub fn document_with_fusion_selection(
        &self,
        optimized: &OptimizedPlan,
        selection: &FusionSelection,
    ) -> Result<TopologyDocument, TopologyViewError> {
        use TopologyViewError::InvalidFusionExplanation as Invalid;
        let plan = &optimized.plan;
        if !optimized.verify()
            || selection.checked_plot_id != plan.checked_plot_id
            || selection.selected_fusion_groups != optimized.fusion_groups
            || selection.selected_candidate_id != selection.selected_realization.candidate_id
            || self
                .current_report()
                .is_none_or(|report| !report.plans.iter().any(|row| row.plan_id == plan.plan_id))
            || selection.considered.is_empty()
            || selection.considered.len() > MAXIMUM_FUSION_CANDIDATES
            || selection.fusion_observations.len() > MAXIMUM_FUSION_OFFERS
        {
            return Err(Invalid);
        }
        let mut ids = BTreeSet::new();
        let mut selected = None;
        for candidate in &selection.considered {
            if !ids.insert(&candidate.candidate_id) {
                return Err(Invalid);
            }
            if candidate.disposition == Disposition::Selected {
                if selected.replace(candidate).is_some() {
                    return Err(Invalid);
                }
            }
            if !matches!(candidate.disposition, Disposition::Rejected(_))
                && candidate
                    .compute_work_units
                    .checked_add(candidate.transport_work_units)
                    != Some(candidate.total_work_units)
            {
                return Err(Invalid);
            }
        }
        let selected = selected.ok_or(Invalid)?;
        if selected.candidate_id != selection.selected_candidate_id
            || selected.fusion_groups != optimized.fusion_groups
        {
            return Err(Invalid);
        }
        let placements = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements);
        let mut count = 0;
        for placement in placements {
            count += 1;
            let Some(choice) = selection
                .selected_realization
                .placements
                .by_gear
                .get(&placement.gear_id)
            else {
                return Err(Invalid);
            };
            if choice.host_id != placement.host_id
                || choice.capability_id != placement.capability_id
            {
                return Err(Invalid);
            }
        }
        if count != selection.selected_realization.placements.by_gear.len()
            || optimized.fusion_groups.len()
                != plan
                    .fragments
                    .iter()
                    .map(|f| f.execution_fusions.len())
                    .sum::<usize>()
        {
            return Err(Invalid);
        }
        let mut document = self.document(None)?;
        push_line(&mut document.lines, format!(
            "FUSION SELECTION plan={} candidate={} historical-at-ms={} work units are planner evidence, not measured latency or resource grants",
            plan.plan_id.as_str(), selection.selected_candidate_id, selection.locality_basis.now_ms))?;
        for candidate in &selection.considered {
            push_line(&mut document.lines, format!(
                "  CANDIDATE {} disposition={:?} compute-work={} transport-work={} transported-bytes={} total-work={} groups={:?} supporting-signs={:?}",
                candidate.candidate_id, candidate.disposition, candidate.compute_work_units,
                candidate.transport_work_units, candidate.transported_bytes, candidate.total_work_units,
                candidate.fusion_groups, candidate.supporting_sign_ids))?;
        }
        for observation in &selection.fusion_observations {
            push_line(
                &mut document.lines,
                format!(
                    "  FUSION EVIDENCE {} work={} provenance={:?}",
                    observation.fusion_id, observation.fused_work_units, observation.provenance
                ),
            )?;
        }
        if document.lines.iter().map(String::len).sum::<usize>() > MAX_RETAINED_REPORT_BYTES {
            return Err(TopologyViewError::FaceTooLarge);
        }
        Ok(document)
    }
}
