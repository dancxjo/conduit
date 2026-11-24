//! Bind the checked resident inventory to its exact Body Plan during preparation.
use super::*;
use conduit_body::BodyPlan;
use conduit_core::ActivePlayId;

impl PatchbayApplicationPort {
    pub fn open_body_residents<'a>(
        plan: &BodyPlan,
        active_play_id: &ActivePlayId,
        subjects: impl ExactSizeIterator<Item = (&'a ExpandedCanonicalPlot, &'a str)>,
        foreground_checked_plot_id: &str,
    ) -> Result<Self, String> {
        if subjects.len() != plan.plots.len() {
            return Err("Patchbay inventory does not cover the Body Plan".into());
        }
        let mut active = Vec::with_capacity(subjects.len());
        for (part, (expanded, title)) in plan.plots.iter().zip(subjects) {
            if expanded.checked_plot_id != part.plot.checked_plot_id
                || expanded.source_document_id != part.plot.source_document_id
            {
                return Err("Patchbay inventory differs from the planned resident Plot".into());
            }
            active.push(
                PatchbayActivePlot::project(
                    expanded,
                    title,
                    part.plot.checked_plot_id.as_str(),
                    part.plan.plan_id.clone(),
                    PatchbayActivePlotState::Playing,
                    part.plot.checked_plot_id.as_str() == foreground_checked_plot_id,
                )
                .map_err(|error| alloc::format!("project resident Patchbay Plot: {error:?}"))?,
            );
        }
        Self::open_active(active, plan.plan_id.clone(), Some(active_play_id.clone()))
            .map_err(|error| alloc::format!("prepare resident Patchbay: {error:?}"))
    }
}
