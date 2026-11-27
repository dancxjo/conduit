//! Owned storage for local fresh State lowering, independent of effect authority.
use super::PlannedStateBoundary;
use alloc::vec::Vec;

impl PlannedStateBoundary {
    /// Requested nested backing of one fresh immutable contract clone. The
    /// enclosing table pays inline Self separately. Retired-cell provenance is
    /// a different ownership path and remains outside this local profile.
    pub fn local_clone_backing_bytes(&self) -> Option<usize> {
        if self.retained.is_some() {
            return None;
        }
        self.state_id
            .as_str()
            .len()
            .checked_add(self.gear_id.as_str().len())?
            .checked_add(self.value_kind.as_str().len())?
            .checked_add(self.initial_value.as_ref().map_or(0, Vec::len))
    }

    /// Actual retained capacities of the same qualified local contract. This
    /// is storage inspection, not State validity, durability or continuation.
    pub fn local_retained_backing_bytes(&self) -> Option<usize> {
        if self.retained.is_some() {
            return None;
        }
        self.state_id
            .0
            .capacity()
            .checked_add(self.gear_id.0.capacity())?
            .checked_add(self.value_kind.0.capacity())?
            .checked_add(self.initial_value.as_ref().map_or(0, Vec::capacity))
    }
}
