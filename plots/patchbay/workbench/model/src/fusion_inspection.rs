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
            if candidate.disposition == Disposition::Selected
                && selected.replace(candidate).is_some()
            {
                return Err(Invalid);
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
                || selection
                    .locality_basis
                    .realization_work
                    .iter()
                    .filter(|work| {
                        work.gear_id == placement.gear_id
                            && work.host_id == placement.host_id
                            && work.boot_id == placement.boot_id
                            && work.capability_id == placement.capability_id
                    })
                    .count()
                    != 1
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
        let mut lines = std::collections::BTreeMap::new();
        for fragment in &plan.fragments {
            for cord in &fragment.connections {
                let Some(line) = &cord.selected_line else {
                    continue;
                };
                let source = plan
                    .fragments
                    .iter()
                    .flat_map(|f| &f.placements)
                    .find(|p| p.placement_id == cord.source_placement_id)
                    .ok_or(Invalid)?;
                let sink = plan
                    .fragments
                    .iter()
                    .flat_map(|f| &f.placements)
                    .find(|p| p.placement_id == cord.sink_placement_id)
                    .ok_or(Invalid)?;
                if lines
                    .insert(
                        (source.gear_id.clone(), sink.gear_id.clone()),
                        line.line_id.clone(),
                    )
                    .is_some_and(|previous| previous != line.line_id)
                {
                    return Err(Invalid);
                }
            }
        }
        if lines != selection.selected_realization.lines {
            return Err(Invalid);
        }
        let mut document = self.document(None)?;
        let mut remaining = 64 * 1024;
        append(&mut document.lines, &mut remaining, format_args!(
            "FUSION SELECTION plan={} candidate={} historical-at-ms={} work units are planner evidence, not measured latency or resource grants",
            plan.plan_id.as_str(), selection.selected_candidate_id, selection.locality_basis.now_ms))?;
        for candidate in &selection.considered {
            append(&mut document.lines, &mut remaining, format_args!(
                "  CANDIDATE {} disposition={:?} compute-work={} transport-work={} transported-bytes={} total-work={} groups={:?} supporting-signs={:?}",
                candidate.candidate_id, candidate.disposition, candidate.compute_work_units,
                candidate.transport_work_units, candidate.transported_bytes, candidate.total_work_units,
                candidate.fusion_groups, candidate.supporting_sign_ids))?;
        }
        for observation in &selection.fusion_observations {
            append(
                &mut document.lines,
                &mut remaining,
                format_args!(
                    "  FUSION EVIDENCE {} work={} provenance={:?}",
                    observation.fusion_id, observation.fused_work_units, observation.provenance
                ),
            )?;
        }
        Ok(document)
    }
}

/// The retained report keeps its existing 96 KiB budget. Borrowed decision
/// details have a separate finite 64 KiB rendering budget and are not retained.
fn append(
    lines: &mut Vec<String>,
    remaining: &mut usize,
    args: std::fmt::Arguments<'_>,
) -> Result<(), TopologyViewError> {
    use std::fmt::Write;
    struct Limited {
        text: String,
        maximum: usize,
    }
    impl Write for Limited {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            if text.len() > self.maximum - self.text.len() {
                return Err(std::fmt::Error);
            }
            self.text.push_str(text);
            Ok(())
        }
    }
    let mut line = Limited {
        text: String::new(),
        maximum: *remaining,
    };
    line.write_fmt(args)
        .map_err(|_| TopologyViewError::FaceTooLarge)?;
    *remaining -= line.text.len();
    push_line(lines, line.text)
}
