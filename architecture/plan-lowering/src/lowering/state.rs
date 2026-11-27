//! Exact finite State identities and storage carried into Host installation.
use super::LoweringError;
use alloc::vec::Vec;
use conduit_core::{PlanFragment, PlannedStateBoundary};
use conduit_kernel::{NodeId, PortId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredState {
    pub slot: u16,
    pub node: NodeId,
    pub next: PortId,
    pub current: PortId,
    pub contract: PlannedStateBoundary,
}

/// Structural lowering only. Installation must still validate the authored kind
/// contract and exact selected implementation, and consume every State entry.
pub(super) fn lower_states(
    fragment: &PlanFragment,
    nodes: &super::PlacementNodes<'_>,
) -> Result<Vec<LoweredState>, LoweringError> {
    // Result collection otherwise starts a geometrically grown minimum table.
    // Reserve the admitted exact State count before cloning any contract owner.
    let mut lowered = Vec::with_capacity(fragment.states.len());
    for (index, state) in fragment.states.iter().enumerate() {
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.gear_id == state.gear_id)
            .ok_or(LoweringError::InvalidFragment)?;
        if placement.inputs.len() != 1 || placement.outputs.len() != 1 {
            return Err(LoweringError::InvalidFragment);
        }
        lowered.push(LoweredState {
            slot: u16::try_from(index).map_err(|_| LoweringError::StateStorageExceeded)?,
            node: nodes
                .get(&placement.placement_id)
                .ok_or(LoweringError::InvalidFragment)?,
            next: PortId(0),
            current: PortId(0),
            contract: state.clone(),
        });
    }
    Ok(lowered)
}
