//! Capture ordinary Body lifecycle evidence from immutable Core verification.
use super::super::verified_inputs::{VerifiedBodyMaskTopology, VerifiedBodyPlotPlan};
use super::super::{
    encoding::bind_body_plan_views,
    masks::validate_mask_views,
    validate_body_time_requirement,
    views::{MaskRef, PlotRef},
    MAX_BODY_PLOTS,
};
use super::*;

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
        Self::capture_prepared_verified(wake, &[], plots, mask_topologies, body_time_requirement)
    }
    pub(crate) fn capture_prepared_verified(
        wake: &Wake,
        prepared: &[PreparedVerifiedBodyPlotPlan],
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
        if plots
            .len()
            .checked_add(prepared.len())
            .is_none_or(|count| count > MAX_BODY_PLOTS)
        {
            return Err(invalid(E::PlotCapacityExceeded));
        }
        let mut plots: Vec<_> = plots
            .iter()
            .map(PlotRef::verified)
            .chain(prepared.iter().map(PlotRef::prepared))
            .collect();
        plots.sort_by(|a, b| a.plot.cmp(b.plot));
        if plots.windows(2).any(|p| p[0].plot >= p[1].plot) {
            return Err(invalid(E::DuplicatePlot));
        }
        for partition in &plots {
            if !wake.workset.contains(partition.plot) {
                return Err(invalid(E::UnexpectedPlot));
            }
            if !partition.plan.valid() || !partition.plan.identity_matches(partition.plot) {
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
        preflight_core_scopes(plots.iter().map(|p| p.plan.scope()).chain(
            masks.iter().flat_map(|m| m.chains()).map(|c| {
                let p = c.plan.metadata();
                (
                    p.fragments.len(),
                    !p.activations.is_empty() || !p.activation_preparations.is_empty(),
                )
            }),
        ))?;
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
            plots: plots.iter().map(|p| p.plan.capture()).collect(),
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
