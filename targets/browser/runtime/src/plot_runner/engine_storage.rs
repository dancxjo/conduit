//! Exact prepared parent storage witness.
use super::*;

/// Exact Plan-bound storage receipt prepared before any kernel step.
#[derive(Debug)]
pub(in crate::plot_runner) struct PreparedStorageWitness {
    pub(in crate::plot_runner) identities: Vec<(conduit_core::PlanId, conduit_core::FragmentId)>,
    pub(in crate::plot_runner) slot_capacities: Vec<u32>,
    pub(in crate::plot_runner) reserved_storage_bytes: usize,
    pub(in crate::plot_runner) live_byte_quota: u32,
    pub(in crate::plot_runner) data_memory_reservation: u64,
}

impl PreparedStorageWitness {
    pub(in crate::plot_runner) fn matches(&self, partitions: &[(&PlanFragment, &LoweredPlanFragment)]) -> bool {
        self.identities.len() == partitions.len()
            && self
                .identities
                .iter()
                .zip(partitions)
                .all(|((plan, fragment), (selected, _))| {
                    plan == &selected.plan_id && fragment == &selected.fragment_id
                })
            && self
                .slot_capacities
                .iter()
                .map(|capacity| u64::from(*capacity))
                .sum::<u64>()
                == u64::from(self.live_byte_quota)
            && self.slot_capacities.len() <= usize::from(BROWSER_VALUE_ITEMS)
            && self.reserved_storage_bytes as u64 + self.data_memory_reservation
                <= u64::from(crate::installed_browser::measurement_limits::MEMORY_POOL_BYTES)
    }
}

