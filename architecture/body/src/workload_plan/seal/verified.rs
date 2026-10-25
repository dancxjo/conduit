//! Capture ordinary Body lifecycle evidence from immutable Core verification.
use super::super::{
    encoding::bind_body_plan_views,
    masks::validate_mask_views,
    validate_body_time_requirement,
    views::{MaskRef, PlotRef},
    MAX_BODY_PLOTS,
};
use super::*;
use crate::ResidentPlot;
use conduit_core::VerifiedPlanMetadata;

#[derive(Debug)]
pub struct VerifiedBodyPlotPlan<'a> {
    pub plot: ResidentPlot,
    pub plan: &'a VerifiedPlanMetadata,
}
#[derive(Debug)]
pub struct VerifiedBodyMaskChain<'a> {
    pub plan: &'a VerifiedPlanMetadata,
    pub stage_placement_ids: Vec<PlacementId>,
}
#[derive(Debug)]
pub struct VerifiedBodyMaskTopology<'a> {
    pub face: BodyFaceSelector,
    pub chains: Vec<VerifiedBodyMaskChain<'a>>,
}
impl BodyPlanSeal {
    /// Same Body workset, Mask path, time and identity rules as ordinary sealing.
    /// Core verification is retained by borrowed opaque inputs; no configuration
    /// is reconstructed and no membership, possession or Play authority is issued.
    pub fn capture_verified(
        wake: &Wake,
        plots: &[VerifiedBodyPlotPlan<'_>],
        mask_topologies: &[VerifiedBodyMaskTopology<'_>],
        body_time_requirement: Option<BodyTimeRequirement>,
    ) -> Result<Self, BodyPlanSealRefusal> {
        use BodyPlanError as E;
        let invalid = BodyPlanSealRefusal::Invalid;
        validate_body_time_requirement(&wake.body_id, body_time_requirement.as_ref())
            .map_err(invalid)?;
        wake.workset
            .validate()
            .map_err(|_| invalid(E::InvalidIdentity))?;
        if wake.workset.is_empty() {
            return Err(invalid(E::EmptyWorkset));
        }
        if plots.len() > MAX_BODY_PLOTS {
            return Err(invalid(E::PlotCapacityExceeded));
        }
        let mut plots: Vec<_> = plots.iter().map(PlotRef::verified).collect();
        plots.sort_by(|a, b| a.plot.cmp(b.plot));
        if plots.windows(2).any(|p| p[0].plot >= p[1].plot) {
            return Err(invalid(E::DuplicatePlot));
        }
        for partition in &plots {
            if !wake.workset.contains(partition.plot) {
                return Err(invalid(E::UnexpectedPlot));
            }
            let plan = partition.plan.metadata();
            if !partition.plan.valid()
                || plan.source_document_id != partition.plot.source_document_id
                || plan.checked_plot_id != partition.plot.checked_plot_id
            {
                return Err(invalid(E::InvalidPlan));
            }
        }
        if wake
            .workset
            .plots()
            .iter()
            .any(|plot| plots.binary_search_by(|p| p.plot.cmp(plot)).is_err())
        {
            return Err(invalid(E::MissingPlot));
        }
        validate_mask_views(
            &wake.workset,
            plots.iter().copied(),
            mask_topologies.iter().map(MaskRef::verified),
        )
        .map_err(invalid)?;
        let mut masks: Vec<_> = mask_topologies.iter().map(MaskRef::verified).collect();
        masks.sort_by(|a, b| a.face.cmp(b.face));
        preflight_core_plans(
            plots.iter().map(|p| p.plan.metadata()).chain(
                masks
                    .iter()
                    .flat_map(|m| m.chains())
                    .map(|c| c.plan.metadata()),
            ),
        )?;
        let plan_id = bind_body_plan_views(
            &wake.body_id,
            &wake.wake_id,
            wake.workload_revision,
            plots.iter().copied(),
            masks.iter().copied(),
            body_time_requirement.as_ref(),
        );
        Ok(Self {
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            plan_id,
            workload_revision: wake.workload_revision,
            workset: wake.workset.clone(),
            plots: plots
                .iter()
                .map(|p| capture_core(p.plan.metadata()))
                .collect(),
            masks: masks
                .iter()
                .map(|m| BodyMaskTopologySeal {
                    face: m.face.clone(),
                    chains: m
                        .chains()
                        .map(|c| BodyMaskChainSeal {
                            plan: capture_core(c.plan.metadata()),
                            stages: c.stages.to_vec(),
                        })
                        .collect(),
                })
                .collect(),
            body_time: body_time_requirement,
        })
    }
}

#[cfg(test)]
mod tests;
