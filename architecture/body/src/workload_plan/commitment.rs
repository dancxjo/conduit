//! Canonical Body Plan bytes shared with ordinary full-Plan sealing.
//! Computed identities are not a Wake, admission, or authority to Start.
use crate::{BodyMaskTopology, BodyPlanError, MAX_BODY_PLOTS};
use conduit_core::BodyTimeRequirement;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug)]
pub struct BodyPlotSeal<'a> {
    pub source_document_id: &'a str,
    pub checked_plot_id: &'a str,
    pub plan_id: &'a str,
}
/// Bounded borrowed pins in the SAME order as BodyPlan::seal's sorted plots.
/// Mask chains and time requirements retain their ordinary typed form.
pub fn body_plan_commitment_digest(
    body_id: &str,
    wake_id: &str,
    workload_revision: u64,
    plots: &[BodyPlotSeal<'_>],
    mask_topologies: &[BodyMaskTopology],
    body_time_requirement: Option<&BodyTimeRequirement>,
) -> Result<[u8; 32], BodyPlanError> {
    let valid_text = |s: &str| !s.is_empty() && u32::try_from(s.len()).is_ok();
    if !valid_text(body_id)
        || !valid_text(wake_id)
        || plots.iter().any(|p| {
            !valid_text(p.source_document_id)
                || !valid_text(p.checked_plot_id)
                || !valid_text(p.plan_id)
        })
    {
        return Err(BodyPlanError::InvalidIdentity);
    }
    if body_time_requirement.is_some_and(|r| !r.valid_basis() || r.body_basis() != body_id) {
        return Err(BodyPlanError::InvalidBodyTimeRequirement);
    }
    if plots.is_empty() {
        return Err(BodyPlanError::EmptyWorkset);
    }
    if plots.len() > MAX_BODY_PLOTS {
        return Err(BodyPlanError::PlotCapacityExceeded);
    }
    if plots.windows(2).any(|p| {
        (p[0].source_document_id, p[0].checked_plot_id)
            >= (p[1].source_document_id, p[1].checked_plot_id)
    }) {
        return Err(BodyPlanError::DuplicatePlot);
    }
    if mask_topologies.len() > crate::MAX_BODY_MASK_TOPOLOGIES
        || mask_topologies.windows(2).any(|p| p[0].face >= p[1].face)
        || mask_topologies.iter().any(|t| {
            t.chains.len() > crate::MAX_BODY_MASK_CHAINS
                || t.chains
                    .iter()
                    .any(|c| c.stage_placement_ids.len() > crate::MAX_BODY_MASK_STAGES)
        })
    {
        return Err(BodyPlanError::InvalidMaskChain);
    }
    Ok(write_body_plan(
        body_id,
        wake_id,
        workload_revision,
        plots.iter().copied(),
        mask_topologies,
        body_time_requirement,
    ))
}
pub(super) fn write_body_plan<'a>(
    body_id: &str,
    wake_id: &str,
    workload_revision: u64,
    plots: impl ExactSizeIterator<Item = BodyPlotSeal<'a>>,
    mask_topologies: &[BodyMaskTopology],
    body_time_requirement: Option<&BodyTimeRequirement>,
) -> [u8; 32] {
    let mut bytes = Sha256::new();
    push(&mut bytes, "conduit.body/body-plan@3");
    push(&mut bytes, body_id);
    push(&mut bytes, wake_id);
    bytes.update(workload_revision.to_le_bytes());
    bytes.update((plots.len() as u32).to_le_bytes());
    for plot in plots {
        push(&mut bytes, plot.source_document_id);
        push(&mut bytes, plot.checked_plot_id);
        push(&mut bytes, plot.plan_id);
    }
    bytes.update((mask_topologies.len() as u32).to_le_bytes());
    for topology in mask_topologies {
        if let Some(plot) = &topology.face.plot {
            push(&mut bytes, "plot");
            push(&mut bytes, plot.source_document_id.as_str());
            push(&mut bytes, plot.checked_plot_id.as_str());
        } else {
            push(&mut bytes, "body");
        }
        match &topology.face.source_placement_id {
            Some(placement) => {
                push(&mut bytes, "plot-placement");
                push(&mut bytes, placement.as_str());
            }
            None => push(&mut bytes, "owner-composed"),
        }
        bytes.update((topology.chains.len() as u32).to_le_bytes());
        for chain in &topology.chains {
            push(&mut bytes, chain.plan.plan_id.as_str());
            bytes.update((chain.stage_placement_ids.len() as u32).to_le_bytes());
            for placement in &chain.stage_placement_ids {
                push(&mut bytes, placement.as_str());
            }
        }
    }
    if let Some(requirement) = body_time_requirement {
        push(&mut bytes, "body-time-requirement@1");
        push(&mut bytes, requirement.body_basis());
        bytes.update(requirement.tolerance().ticks().to_le_bytes());
        push_temporal_scale(&mut bytes, requirement.tolerance().scale());
        bytes.update(requirement.horizon().ticks().to_le_bytes());
        push_temporal_scale(&mut bytes, requirement.horizon().scale());
    }
    bytes.finalize().into()
}
fn push(output: &mut Sha256, value: &str) {
    output.update((value.len() as u32).to_le_bytes());
    output.update(value.as_bytes());
}
fn push_temporal_scale(bytes: &mut Sha256, scale: conduit_core::TemporalScale) {
    let label = match scale {
        conduit_core::TemporalScale::Seconds => "seconds",
        conduit_core::TemporalScale::Milliseconds => "milliseconds",
        conduit_core::TemporalScale::Microseconds => "microseconds",
        conduit_core::TemporalScale::Nanoseconds => "nanoseconds",
    };
    push(bytes, label);
}
