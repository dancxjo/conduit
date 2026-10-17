//! Pre-Play validation of the installed physical chain Back.
use crate::pure_chain_fusion::{
    PURE_CHAIN_FUSION_ARTIFACT, PURE_CHAIN_FUSION_IMPLEMENTATION, PURE_CHAIN_FUSION_PROFILE,
};
use conduit_core::PlanFragment;
use conduit_kernel::{CordEndpoint, NodeId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn prepare_chains(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Vec<Vec<NodeId>>, String> {
    let mut chains = Vec::with_capacity(lowered.fusions.len());
    for fusion in &lowered.fusions {
        if fusion.implementation_id.as_str() != PURE_CHAIN_FUSION_IMPLEMENTATION
            || fusion.execution_profile_id.as_str() != PURE_CHAIN_FUSION_PROFILE
            || fusion.artifact_id.as_str() != PURE_CHAIN_FUSION_ARTIFACT
            || !(2..=32).contains(&fusion.nodes.len())
            || fusion.cords.len() + 1 != fusion.nodes.len()
        {
            return Err("unsupported std physical fusion Back or chain envelope".into());
        }
        for node in &fusion.nodes {
            let identity = lowered
                .nodes
                .iter()
                .find(|n| n.node == *node)
                .ok_or("fusion node absent")?;
            let placement = fragment
                .placements
                .iter()
                .find(|p| p.placement_id == identity.placement_id)
                .ok_or("fusion placement absent")?;
            super::pure_expression_back::local_budget(placement)?;
            crate::pure_chain_fusion::validate_program_profile(
                &super::pure_expression_back::program_from_placement(placement)?,
                placement.inputs[0].temporal,
            )?;
            if !fragment.execution_regions.is_empty()
                && !fragment.execution_regions.iter().any(|r| {
                    !r.preemption_required
                        && !r.isolation_required
                        && fusion.nodes.iter().all(|node| {
                            lowered
                                .nodes
                                .iter()
                                .find(|n| n.node == *node)
                                .is_some_and(|n| r.admitted_placements.contains(&n.placement_id))
                        })
                })
            {
                return Err("fusion crosses an execution region or isolation boundary".into());
            }
        }
        let edges: Vec<_> = fusion
            .cords
            .iter()
            .map(|id| {
                let cord = lowered
                    .cords
                    .iter()
                    .find(|c| c.spec.cord == *id)
                    .ok_or("fusion Cord absent")?;
                match (cord.spec.source, cord.spec.sink) {
                    (
                        CordEndpoint::Local { node: source, .. },
                        CordEndpoint::Local { node: sink, .. },
                    ) if fusion.nodes.contains(&source) && fusion.nodes.contains(&sink) => {
                        Ok((source, sink))
                    }
                    _ => Err("fusion crosses a Host boundary"),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let starts: Vec<_> = fusion
            .nodes
            .iter()
            .copied()
            .filter(|n| !edges.iter().any(|(_, sink)| sink == n))
            .collect();
        if starts.len() != 1 {
            return Err("fusion requires a unique chain entry".into());
        }
        let mut ordered = Vec::with_capacity(fusion.nodes.len());
        let mut current = starts[0];
        loop {
            if ordered.contains(&current) {
                return Err("fusion contains a cycle".into());
            }
            ordered.push(current);
            let next: Vec<_> = edges
                .iter()
                .filter(|(source, _)| *source == current)
                .collect();
            if next.is_empty() {
                break;
            }
            if next.len() != 1 {
                return Err("fusion crosses fan-out".into());
            }
            current = next[0].1;
        }
        if ordered.len() != fusion.nodes.len() {
            return Err("fusion is disconnected".into());
        }
        chains.push(ordered);
    }
    Ok(chains)
}

pub(super) fn step_failure(
    error: conduit_kernel::scheduler::SchedulerError,
    node: Option<NodeId>,
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> String {
    let placement = node
        .and_then(|node| lowered.nodes.iter().find(|n| n.node == node))
        .and_then(|node| {
            fragment
                .placements
                .iter()
                .find(|p| p.placement_id == node.placement_id)
        });
    match placement {
        Some(placement) => format!("installed kernel step: {error:?}; logical-gear={} placement={} source-document={} source-span={:?}",
            placement.gear_id.as_str(), placement.placement_id.as_str(), fragment.source_document_id.as_str(), placement.source_span),
        None => format!("installed kernel step: {error:?}"),
    }
}
