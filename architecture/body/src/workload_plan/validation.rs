//! Revalidate by borrowing rich Plot Plans. Only bounded references are sorted.
use super::{
    bind_body_plan, validate_body_time_requirement, validate_mask_topologies, verify_plan,
    BodyPlan, BodyPlanError as E, Wake, MAX_BODY_PLOTS,
};
use alloc::vec::Vec;

impl BodyPlan {
    pub fn validate_for(&self, wake: &Wake) -> Result<(), E> {
        if self.body_id != wake.body_id {
            return Err(E::WrongBody);
        }
        if self.wake_id != wake.wake_id {
            return Err(E::WrongWake);
        }
        if self.workload_revision != wake.workload_revision || self.workset != wake.workset {
            return Err(E::StaleWorkload);
        }
        // Match seal_with_requirements' validation and canonical order without
        // cloning its nested Plans, their semantic types, or computation bytes.
        validate_body_time_requirement(&wake.body_id, self.body_time_requirement.as_ref())?;
        wake.workset.validate().map_err(|_| E::InvalidIdentity)?;
        if wake.workset.is_empty() {
            return Err(E::EmptyWorkset);
        }
        if self.plots.len() > MAX_BODY_PLOTS {
            return Err(E::PlotCapacityExceeded);
        }
        let mut plots: Vec<_> = self.plots.iter().collect();
        plots.sort_by(|left, right| left.plot.cmp(&right.plot));
        if plots.windows(2).any(|pair| pair[0].plot >= pair[1].plot) {
            return Err(E::DuplicatePlot);
        }
        for partition in &plots {
            if !wake.workset.contains(&partition.plot) {
                return Err(E::UnexpectedPlot);
            }
            if !verify_plan(&partition.plan)
                || partition.plan.source_document_id != partition.plot.source_document_id
                || partition.plan.checked_plot_id != partition.plot.checked_plot_id
            {
                return Err(E::InvalidPlan);
            }
        }
        if wake.workset.plots().iter().any(|plot| {
            plots
                .binary_search_by(|value| value.plot.cmp(plot))
                .is_err()
        }) {
            return Err(E::MissingPlot);
        }
        validate_mask_topologies(&wake.workset, &self.plots, &self.mask_topologies)?;
        // validate_mask_topologies enforces the finite topology capacity before
        // this reference array is allocated. Chain Plans remain borrowed too.
        let mut masks: Vec<_> = self.mask_topologies.iter().collect();
        masks.sort_by(|left, right| left.face.cmp(&right.face));
        if self.plan_id
            != bind_body_plan(
                &wake.body_id,
                &wake.wake_id,
                wake.workload_revision,
                plots.into_iter(),
                masks.into_iter(),
                self.body_time_requirement.as_ref(),
            )
        {
            return Err(E::InvalidIdentity);
        }
        Ok(())
    }
}
