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

/// Joins a consumed value to the exact prior routing event through the planned
/// Cord. Event order only selects among events already proven to be endpoints
/// of that Cord; adjacency alone never creates the causal edge.
pub(super) fn record_planned_transfer_edges(
    graph: &mut CausalEvidence<MAXIMUM_BODY_CAUSAL_EDGES>,
    nodes: &[BodyCausalNode],
    plan: &BodyPlan,
    maps: &[KernelIdentityMap],
) -> Result<(), BodyCausalEvidenceRefusal> {
    for (index, consumed) in nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kernel_kind == KernelEventKind::ValueConsumed)
    {
        let Some(sink_port) = consumed.kernel_port else {
            return Err(BodyCausalEvidenceRefusal::MismatchedIdentityMap);
        };
        let Some((map, sink_port)) = maps.iter().find_map(|map| {
            let node = map.node_for_placement(&consumed.placement_id)?;
            let identity = map.port_identity(node, PortDirection::Input, sink_port)?;
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
            connection.sink_placement_id == consumed.placement_id
                && connection.sink_port_id == sink_port
        });
        let Some(connection) = connections.next() else {
            // External Face input or evidence retained outside this run has no
            // same-fragment routing node to join here.
            continue;
        };
        if connections.next().is_some() {
            return Err(BodyCausalEvidenceRefusal::MismatchedExecution);
        }
        let Some(routed) = nodes[..index].iter().rev().find(|candidate| {
            if candidate.placement_id != connection.source_placement_id
                || candidate.kernel_kind != KernelEventKind::ValueRouted
            {
                return false;
            }
            let Some(port) = candidate.kernel_port else {
                return false;
            };
            maps.iter().any(|source_map| {
                source_map
                    .node_for_placement(&candidate.placement_id)
                    .and_then(|node| source_map.port_identity(node, PortDirection::Output, port))
                    .is_some_and(|identity| identity.port_id == connection.source_port_id)
            })
        }) else {
            // A planned Cord may cross an evidence-retention boundary. Missing
            // history remains missing rather than becoming a guessed edge.
            continue;
        };
        graph.record(CausalEdge {
            effect: consumed.evidence,
            relationship: CausalRelationship::DerivedFrom,
            cause: routed.evidence,
        })?;
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
