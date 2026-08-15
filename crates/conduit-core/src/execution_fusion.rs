use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    ArtifactId, ConnectionId, ExecutionProfileId, FusionId, ImplementationId, PlacementId,
    PlanFragment,
};

/// One exact Host-offered execution optimization over ordinary semantic
/// placements and Cords. The referenced placements and connections remain in
/// the Plan and retain the kernel's typed pressure, cancellation, and Sign
/// semantics; this record permits an implementation to realize them as one
/// local execution unit without creating a second semantic graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedFusion {
    pub fusion_id: FusionId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub preserved_placements: Vec<PlacementId>,
    pub preserved_connections: Vec<ConnectionId>,
}

pub(crate) fn verify(fragment: &PlanFragment) -> bool {
    let mut fusion_ids = BTreeSet::new();
    let mut fused_placements = BTreeSet::new();
    fragment.execution_fusions.iter().all(|fusion| {
        fusion_ids.insert(&fusion.fusion_id)
            && !fusion.fusion_id.as_str().is_empty()
            && !fusion.execution_profile_id.as_str().is_empty()
            && !fusion.implementation_id.as_str().is_empty()
            && !fusion.artifact_id.as_str().is_empty()
            && fusion.preserved_placements.len() >= 2
            && fusion
                .preserved_placements
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && fusion
                .preserved_connections
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && fusion.preserved_placements.iter().all(|placement_id| {
                fused_placements.insert(placement_id)
                    && fragment
                        .placements
                        .iter()
                        .any(|placement| &placement.placement_id == placement_id)
            })
            && fusion.preserved_connections.iter().all(|connection_id| {
                fragment.connections.iter().any(|connection| {
                    &connection.connection_id == connection_id
                        && fusion
                            .preserved_placements
                            .contains(&connection.source_placement_id)
                        && fusion
                            .preserved_placements
                            .contains(&connection.sink_placement_id)
                        && connection.selected_line.is_none()
                        && connection.admitted_lines.is_empty()
                })
            })
    })
}

pub(crate) fn push_canonical(output: &mut Vec<u8>, fusions: &[PlannedFusion]) {
    if fusions.is_empty() {
        return;
    }
    push_u32(output, fusions.len() as u32);
    for fusion in fusions {
        push_string(output, fusion.fusion_id.as_str());
        push_string(output, fusion.execution_profile_id.as_str());
        push_string(output, fusion.implementation_id.as_str());
        push_string(output, fusion.artifact_id.as_str());
        push_u32(output, fusion.preserved_placements.len() as u32);
        for placement in &fusion.preserved_placements {
            push_string(output, placement.as_str());
        }
        push_u32(output, fusion.preserved_connections.len() as u32);
        for connection in &fusion.preserved_connections {
            push_string(output, connection.as_str());
        }
    }
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_string(output: &mut Vec<u8>, value: &str) {
    push_u32(output, value.len() as u32);
    output.extend_from_slice(value.as_bytes());
}
