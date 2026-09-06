//! Exact finite State identities and storage carried into Host installation.
use super::LoweringError;
use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::{PlacementId, PlanFragment, PlannedStateBoundary};
use conduit_kernel::{NodeId, PortId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredState {
    pub slot: u16,
    pub node: NodeId,
    pub next: PortId,
    pub current: PortId,
    pub contract: PlannedStateBoundary,
}

/// Structural lowering only. Installation must still validate the authored Kind
/// contract and exact selected implementation, and consume every State entry.
pub(super) fn lower_states(
    fragment: &PlanFragment,
    nodes: &BTreeMap<PlacementId, NodeId>,
) -> Result<Vec<LoweredState>, LoweringError> {
    fragment
        .states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.gear_id == state.gear_id)
                .ok_or(LoweringError::InvalidFragment)?;
            // Core validation requires exactly one typed input and output.
            if placement.inputs.len() != 1 || placement.outputs.len() != 1 {
                return Err(LoweringError::InvalidFragment);
            }
            Ok(LoweredState {
                slot: u16::try_from(index).map_err(|_| LoweringError::StateStorageExceeded)?,
                node: *nodes
                    .get(&placement.placement_id)
                    .ok_or(LoweringError::InvalidFragment)?,
                next: PortId(0),
                current: PortId(0),
                contract: state.clone(),
            })
        })
        .collect()
}
