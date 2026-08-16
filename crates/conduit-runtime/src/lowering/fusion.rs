use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use conduit_core::{
    ArtifactId, ExecutionProfileId, FusionId, ImplementationId, PlacementId, PlanFragment,
};
use conduit_kernel::{CordId, NodeId};

use super::{LoweredCord, LoweringError};

/// Numeric binding for a selected fusion. Nodes and Cords remain ordinary
/// kernel identities; the fused implementation receives no alternate graph or
/// pressure/cancellation protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredFusion {
    pub fusion_id: FusionId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub nodes: Vec<NodeId>,
    pub cords: Vec<CordId>,
}

pub(super) fn lower_fusions(
    fragment: &PlanFragment,
    placement_nodes: &BTreeMap<PlacementId, NodeId>,
    cords: &[LoweredCord],
) -> Result<Vec<LoweredFusion>, LoweringError> {
    fragment
        .execution_fusions
        .iter()
        .map(|fusion| {
            let nodes = fusion
                .preserved_placements
                .iter()
                .map(|placement| {
                    placement_nodes
                        .get(placement)
                        .copied()
                        .ok_or(LoweringError::InvalidFragment)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let fusion_cords = fusion
                .preserved_connections
                .iter()
                .map(|connection| {
                    cords
                        .iter()
                        .find(|cord| &cord.connection_id == connection)
                        .map(|cord| cord.spec.cord)
                        .ok_or(LoweringError::InvalidFragment)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(LoweredFusion {
                fusion_id: fusion.fusion_id.clone(),
                execution_profile_id: fusion.execution_profile_id.clone(),
                implementation_id: fusion.implementation_id.clone(),
                artifact_id: fusion.artifact_id.clone(),
                nodes,
                cords: fusion_cords,
            })
        })
        .collect()
}
