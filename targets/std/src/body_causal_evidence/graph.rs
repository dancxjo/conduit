use super::{
    BodyCausalEvidenceRefusal, BodyCausalNode, CausalEdge, CausalEvidence, CausalRelationship,
    KernelEventKind, MAXIMUM_BODY_CAUSAL_EDGES,
};

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
