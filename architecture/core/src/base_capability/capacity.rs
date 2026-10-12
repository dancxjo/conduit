//! Read-only finite retained-slot geometry; this information grants no possession.
use super::{BaseCapabilityTable, BaseOperationClaim};

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

// Each allocation is charged separately using the caller's allocator granule.
// These measurements describe storage only and do not establish possession.
fn rounded(bytes: usize, unit: usize) -> Option<usize> {
    if !unit.is_power_of_two() || unit < core::mem::align_of::<super::CapabilityEntry>() {
        return None;
    }
    bytes.checked_add(unit - 1).map(|value| value & !(unit - 1))
}

impl BaseCapabilityTable {
    /// Inline storage and actual capacities of every retained allocation.
    pub fn retained_storage_bytes(&self, allocation_unit: usize) -> Option<usize> {
        let mut bytes = core::mem::size_of::<Self>();
        bytes = bytes.checked_add(rounded(
            self.entries
                .capacity()
                .checked_mul(core::mem::size_of::<super::CapabilityEntry>())?,
            allocation_unit,
        )?)?;
        for text in [&self.host_id.0, &self.boot_id.0, &self.base_instance_id.0] {
            bytes = bytes.checked_add(rounded(text.capacity(), allocation_unit)?)?;
        }
        for entry in &self.entries {
            bytes = bytes.checked_add(rounded(
                entry
                    .active_sequences
                    .capacity()
                    .checked_mul(core::mem::size_of::<u32>())?,
                allocation_unit,
            )?)?;
            let scope = &entry.inspection.scope;
            for text in [
                &entry.inspection.possession_id.0,
                &scope.host_id.0,
                &scope.boot_id.0,
                &scope.base_instance_id.0,
                &scope.plan_id.0,
                &scope.active_play_id.0,
                &scope.authority_grant_id.0,
                &scope.authority_contract_id.0,
                &scope.capability_id.0,
                &scope.implementation_id.0,
                &scope.operation_contract_id.0,
                &scope.subject_kind.0,
                &scope.resource_pool_id.0,
                &scope.resource_generation_id.0,
                &scope.envelope_id.0,
            ] {
                bytes = bytes.checked_add(rounded(text.capacity(), allocation_unit)?)?;
            }
        }
        Some(bytes)
    }
}

impl BaseOperationClaim {
    /// Inline storage and actual capacities of all retained identity strings.
    pub fn retained_storage_bytes(&self, allocation_unit: usize) -> Option<usize> {
        let mut bytes = core::mem::size_of::<Self>();
        for text in [
            &self.host_id.0,
            &self.boot_id.0,
            &self.base_instance_id.0,
            &self.plan_id.0,
            &self.active_play_id.0,
            &self.implementation_id.0,
            &self.operation_contract_id.0,
            &self.subject_kind.0,
            &self.resource_pool_id.0,
            &self.resource_generation_id.0,
            &self.envelope_id.0,
        ] {
            bytes = bytes.checked_add(rounded(text.capacity(), allocation_unit)?)?;
        }
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BaseInstanceId, BootId, HostId};

    #[test]
    fn hidden_identifier_capacity_is_charged_without_changing_identity() {
        let mut host = alloc::string::String::with_capacity(65536);
        host.push_str("fixture/host");
        let table = BaseCapabilityTable::new(
            HostId(host),
            BootId::from("fixture/boot"),
            BaseInstanceId::from("fixture/base"),
            1,
            [71; 32],
            1,
        )
        .unwrap();
        assert!(table.retained_storage_bytes(32).unwrap() >= 65536);
        assert_eq!(table.host_id.as_str(), "fixture/host");
        assert_eq!(table.retained_storage_bytes(3), None);
        assert_eq!(rounded(usize::MAX, 32), None);
    }

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
