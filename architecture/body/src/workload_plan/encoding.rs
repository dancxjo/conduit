//! One historical Body Plan canonical encoder for both preparation representations.
use super::*;
pub(super) fn bind_body_plan_views<'a>(
    body_id: &BodyId,
    wake_id: &WakeId,
    workload_revision: u64,
    plots: impl ExactSizeIterator<Item = views::PlotRef<'a>>,
    mask_topologies: impl ExactSizeIterator<Item = views::MaskRef<'a>>,
    body_time_requirement: Option<&BodyTimeRequirement>,
) -> PlanId {
    let mut bytes = Vec::new();
    push(&mut bytes, "conduit.body/body-plan@3");
    push(&mut bytes, body_id.as_str());
    push(&mut bytes, wake_id.as_str());
    bytes.extend_from_slice(&workload_revision.to_le_bytes());
    bytes.extend_from_slice(&(plots.len() as u32).to_le_bytes());
    for plot in plots {
        push(&mut bytes, plot.plot.source_document_id.as_str());
        push(&mut bytes, plot.plot.checked_plot_id.as_str());
        push(&mut bytes, plot.plan.metadata().plan_id.as_str());
    }
    bytes.extend_from_slice(&(mask_topologies.len() as u32).to_le_bytes());
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
        bytes.extend_from_slice(&(topology.chains().len() as u32).to_le_bytes());
        for chain in topology.chains() {
            push(&mut bytes, chain.plan.metadata().plan_id.as_str());
            bytes.extend_from_slice(&(chain.stages.len() as u32).to_le_bytes());
            for placement in chain.stages {
                push(&mut bytes, placement.as_str());
            }
        }
    }
    if let Some(requirement) = body_time_requirement {
        push(&mut bytes, "body-time-requirement@1");
        push(&mut bytes, requirement.body_basis());
        bytes.extend_from_slice(&requirement.tolerance().ticks().to_le_bytes());
        push_temporal_scale(&mut bytes, requirement.tolerance().scale());
        bytes.extend_from_slice(&requirement.horizon().ticks().to_le_bytes());
        push_temporal_scale(&mut bytes, requirement.horizon().scale());
    }
    PlanId::from(digest_id("body-plan", &bytes))
}
