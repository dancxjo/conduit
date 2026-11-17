//! The existing body-wide Plan/Play canonical transcript, streamed without scratch Vecs.
use super::*;
use alloc::string::String;
use sha2::{Digest, Sha256};
struct CanonicalHash(Sha256);
impl CanonicalHash {
    fn new() -> Self {
        Self(Sha256::new())
    }
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}
pub(super) fn bind_body_plan(
    body_id: &BodyId,
    wake_id: &WakeId,
    workload_revision: u64,
    plots: &[BodyPlotPlan],
    mask_topologies: &[BodyMaskTopology],
    body_time_requirement: Option<&BodyTimeRequirement>,
) -> PlanId {
    bind_body_plan_views(
        body_id,
        wake_id,
        workload_revision,
        plots.iter(),
        mask_topologies.iter(),
        body_time_requirement,
    )
}
pub(super) fn bind_body_plan_views<'a>(
    body_id: &BodyId,
    wake_id: &WakeId,
    workload_revision: u64,
    plots: impl ExactSizeIterator<Item = &'a BodyPlotPlan>,
    mask_topologies: impl ExactSizeIterator<Item = &'a BodyMaskTopology>,
    body_time_requirement: Option<&BodyTimeRequirement>,
) -> PlanId {
    let mut bytes = CanonicalHash::new();
    push(&mut bytes, "conduit.body/body-plan@3");
    push(&mut bytes, body_id.as_str());
    push(&mut bytes, wake_id.as_str());
    bytes.extend_from_slice(&workload_revision.to_le_bytes());
    bytes.extend_from_slice(&(plots.len() as u32).to_le_bytes());
    for plot in plots {
        push(&mut bytes, plot.plot.source_document_id.as_str());
        push(&mut bytes, plot.plot.checked_plot_id.as_str());
        push(&mut bytes, plot.plan.plan_id.as_str());
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
        bytes.extend_from_slice(&(topology.chains.len() as u32).to_le_bytes());
        for chain in &topology.chains {
            push(&mut bytes, chain.plan.plan_id.as_str());
            bytes.extend_from_slice(&(chain.stage_placement_ids.len() as u32).to_le_bytes());
            for placement in &chain.stage_placement_ids {
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
    PlanId::from(digest_id("body-plan", bytes))
}

fn push_temporal_scale(bytes: &mut CanonicalHash, scale: conduit_core::TemporalScale) {
    let label = match scale {
        conduit_core::TemporalScale::Seconds => "seconds",
        conduit_core::TemporalScale::Milliseconds => "milliseconds",
        conduit_core::TemporalScale::Microseconds => "microseconds",
        conduit_core::TemporalScale::Nanoseconds => "nanoseconds",
    };
    push(bytes, label);
}

pub(super) fn bind_body_play(
    body_id: &BodyId,
    wake_id: &WakeId,
    plan_id: &PlanId,
    play_sequence: u64,
) -> ActivePlayId {
    let mut bytes = CanonicalHash::new();
    push(&mut bytes, "conduit.body/body-play@1");
    push(&mut bytes, body_id.as_str());
    push(&mut bytes, wake_id.as_str());
    push(&mut bytes, plan_id.as_str());
    bytes.extend_from_slice(&play_sequence.to_le_bytes());
    ActivePlayId::from(digest_id("body-play", bytes))
}

fn push(output: &mut CanonicalHash, value: &str) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}
fn digest_id(prefix: &str, bytes: CanonicalHash) -> String {
    let digest = bytes.0.finalize();
    let mut encoded = [0; 64];
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (pair, byte) in encoded.as_chunks_mut::<2>().0.iter_mut().zip(digest) {
        pair[0] = HEX[usize::from(byte >> 4)];
        pair[1] = HEX[usize::from(byte & 15)];
    }
    let mut output = String::with_capacity(prefix.len() + "/sha256:".len() + encoded.len());
    output.push_str(prefix);
    output.push_str("/sha256:");
    output.push_str(core::str::from_utf8(&encoded).expect("hexadecimal identity"));
    output
}
#[cfg(test)]
mod tests;
