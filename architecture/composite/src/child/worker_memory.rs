//! Retained backing of a worker child without external boundary maps.
use super::*;
impl ChildKernel {
    pub(crate) fn retained_worker_bytes(&self) -> Option<usize> {
        // Region workers have no external boundary/Fore maps. Do not silently
        // omit their allocations if this method is used by another caller.
        if !self.boundaries.is_empty() || !self.input_targets.is_empty() {
            return None;
        }
        Self::fixed_storage_floor_bytes()
            .checked_add(self.scheduler.values().reserved_storage_bytes())?
            .checked_add(self.scheduler.signs().reserved_storage_bytes())?
            .checked_add(self.worker_back_bytes?)
    }
}
