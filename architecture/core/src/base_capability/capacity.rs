//! Read-only finite retained-slot geometry; this information grants no possession.
use super::BaseCapabilityTable;

impl BaseCapabilityTable {
    /// Logical issuance limit established when Root creates this table.
    pub fn maximum_capabilities(&self) -> u16 {
        self.maximum_capabilities
    }

    /// Actual retained Vec capacity, including any allocator-provided surplus.
    /// Inspection count describes live entries and cannot establish this bound.
    pub fn reserved_capability_slots(&self) -> usize {
        self.entries.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BaseInstanceId, BootId, HostId};

    #[test]
    fn retained_slots_exist_even_when_no_capability_has_been_issued() {
        for limit in [1, 4, 32] {
            let table = BaseCapabilityTable::new(
                HostId::from("fixture/host"),
                BootId::from("fixture/boot"),
                BaseInstanceId::from("fixture/base"),
                1,
                [71; 32],
                limit,
            )
            .unwrap();
            assert_eq!(table.inspections().count(), 0);
            assert_eq!(table.maximum_capabilities(), limit);
            assert!(table.reserved_capability_slots() >= usize::from(limit));
        }
    }
}
