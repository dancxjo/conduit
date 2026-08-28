//! Rejoin one validated set of host-neutral region states.

use alloc::vec;
use alloc::vec::Vec;

use crate::{
    PartitionedReactionDiffusionGeneration, ReactionDiffusionCell,
    ReactionDiffusionPartitionRefusal,
};

pub(crate) fn join_region_cells(
    generation: &PartitionedReactionDiffusionGeneration,
) -> Result<Vec<ReactionDiffusionCell>, ReactionDiffusionPartitionRefusal> {
    let mut joined = vec![
        ReactionDiffusionCell::REST;
        usize::from(generation.width) * usize::from(generation.height)
    ];
    for region_state in &generation.regions {
        let region = region_state.region;
        for local_y in 0..usize::from(region.height) {
            for local_x in 0..usize::from(region.width) {
                let global_x = usize::from(region.origin_x) + local_x;
                let global_y = usize::from(region.origin_y) + local_y;
                let local_index = local_y * usize::from(region.width) + local_x;
                let Some(value) = region_state.cells.get(local_index) else {
                    return Err(ReactionDiffusionPartitionRefusal::RegionStateMismatch);
                };
                joined[global_y * usize::from(generation.width) + global_x] = *value;
            }
        }
    }
    Ok(joined)
}
