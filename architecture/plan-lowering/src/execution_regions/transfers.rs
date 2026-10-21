//! Typed cross-region transfers are projections of exact sealed local Cords.
use super::RegionPreparationError;
use crate::lowering::LoweredPlanFragment;
use alloc::vec::Vec;
use conduit_core::{PlanFragment, PlannedConnection};
use conduit_kernel::CordId;

#[derive(Debug)]
pub struct PreparedRegionTransfer<'plan> {
    connection: &'plan PlannedConnection,
    cord: CordId,
    source_region: usize,
    sink_region: usize,
}
impl<'plan> PreparedRegionTransfer<'plan> {
    pub fn connection(&self) -> &'plan PlannedConnection {
        self.connection
    }
    pub fn cord(&self) -> CordId {
        self.cord
    }
    pub fn source_region(&self) -> usize {
        self.source_region
    }
    pub fn sink_region(&self) -> usize {
        self.sink_region
    }
}

pub(super) fn prepare<'plan>(
    fragment: &'plan PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Vec<PreparedRegionTransfer<'plan>>, RegionPreparationError> {
    let mut transfers = Vec::new();
    for connection in &fragment.connections {
        let region_for = |placement| {
            fragment
                .execution_regions
                .iter()
                .position(|r| r.admitted_placements.contains(placement))
                .ok_or(RegionPreparationError::UnsupportedCrossRegionDependency)
        };
        let source_region = region_for(&connection.source_placement_id)?;
        let sink_region = region_for(&connection.sink_placement_id)?;
        if source_region == sink_region {
            continue;
        }
        let cord = lowered
            .identity
            .cord_for_connection(&connection.connection_id)
            .ok_or(RegionPreparationError::UnsupportedCrossRegionDependency)?;
        transfers.push(PreparedRegionTransfer {
            connection,
            cord,
            source_region,
            sink_region,
        });
    }
    Ok(transfers)
}
