use super::{
    BodyCausalEvidenceRefusal, BodyCausalNode, BodyPlan, CausalEdge, CausalEvidence,
    CausalRelationship, KernelEventKind, KernelIdentityMap, MAXIMUM_BODY_CAUSAL_EDGES,
};
use conduit_core::{ConnectionTrack, PortDirection};

pub(super) fn record_intra_run_edges(
    graph: &mut CausalEvidence<MAXIMUM_BODY_CAUSAL_EDGES>,
    nodes: &[BodyCausalNode],
) -> Result<(), BodyCausalEvidenceRefusal> {
    for (index, effect) in nodes.iter().enumerate() {
        if let Some(cause) = nodes[..index].iter().rev().find(|candidate| {
            candidate.play_id == effect.play_id && candidate.placement_id == effect.placement_id
        }) {
            graph.record(CausalEdge {
                effect: effect.evidence,
                relationship: relationship(cause.kernel_kind, effect.kernel_kind),
                cause: cause.evidence,
            })?;
        }
    }
    Ok(())
}

/// Joins recovery to its abnormal source through the exact planned Cord.
/// Event sequence only selects the still-unresolved observation on that
/// already-proven topology; it never establishes causality by adjacency.
pub(super) fn record_planned_recovery_edges(
    graph: &mut CausalEvidence<MAXIMUM_BODY_CAUSAL_EDGES>,
    nodes: &[BodyCausalNode],
    plan: &BodyPlan,
    maps: &[KernelIdentityMap],
) -> Result<(), BodyCausalEvidenceRefusal> {
    for (index, recovered) in nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kernel_kind == KernelEventKind::SemanticAbnormalRecovered)
    {
        let Some(port) = recovered.kernel_port else {
            return Err(BodyCausalEvidenceRefusal::MismatchedIdentityMap);
        };
        let Some((map, sink_port)) = maps.iter().find_map(|map| {
            let node = map.node_for_placement(&recovered.placement_id)?;
            let identity = map.port_identity(node, PortDirection::Input, port)?;
            Some((map, identity.port_id.clone()))
        }) else {
            return Err(BodyCausalEvidenceRefusal::MismatchedIdentityMap);
        };
        let Some(fragment) = plan
            .forms
            .iter()
            .flat_map(|form| &form.plan.fragments)
            .find(|fragment| {
                fragment.plan_id == map.plan_id && fragment.fragment_id == map.fragment_id
            })
        else {
            return Err(BodyCausalEvidenceRefusal::MismatchedIdentityMap);
        };
        let mut connections = fragment.connections.iter().filter(|connection| {
            connection.track == ConnectionTrack::AbnormalTerminal
                && connection.sink_placement_id == recovered.placement_id
                && connection.sink_port_id == sink_port
        });
        let Some(connection) = connections.next() else {
            return Err(BodyCausalEvidenceRefusal::MismatchedExecution);
        };
        if connections.next().is_some() {
            return Err(BodyCausalEvidenceRefusal::MismatchedExecution);
        }
        let Some(cause) = nodes[..index].iter().rev().find(|candidate| {
            candidate.placement_id == connection.source_placement_id
                && candidate.kernel_kind == KernelEventKind::SemanticAbnormal
        }) else {
            return Err(BodyCausalEvidenceRefusal::MismatchedExecution);
        };
        graph.record(CausalEdge {
            effect: recovered.evidence,
            relationship: CausalRelationship::Corrects,
            cause: cause.evidence,
        })?;
    }
    Ok(())
}

fn relationship(cause: KernelEventKind, effect: KernelEventKind) -> CausalRelationship {
    if cause == KernelEventKind::SemanticAbnormal
        && effect == KernelEventKind::SemanticAbnormalRecovered
    {
        CausalRelationship::Corrects
    } else if effect == KernelEventKind::SemanticAbnormal {
        CausalRelationship::TerminatedBecause
    } else {
        CausalRelationship::DerivedFrom
    }
}
