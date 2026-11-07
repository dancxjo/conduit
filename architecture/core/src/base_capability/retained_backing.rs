//! Exact heap payload inspection, distinct from rounded allocator reservations.
use super::*;
use core::mem::size_of;
impl BaseCapabilityTable {
    /// Requested table slots plus one issuance's sequence/digest buffers.
    /// Identity strings, inline owners and allocator overhead are separate.
    /// Read-only geometry: this creates neither a table nor possession.
    pub fn single_issuance_backing_bytes(
        maximum_capabilities: u16,
        maximum_in_flight: u16,
    ) -> Option<usize> {
        if maximum_capabilities == 0 || maximum_in_flight == 0 {
            return None;
        }
        usize::from(maximum_capabilities)
            .checked_mul(size_of::<CapabilityEntry>())?
            .checked_add(usize::from(maximum_in_flight).checked_mul(size_of::<u32>())?)?
            .checked_add(64)
    }

    /// Actual owned Vec/String capacities only; excludes inline Self, allocator
    /// overhead and caller-owned bearer/lease values. Does not issue authority.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = self
            .entries
            .capacity()
            .checked_mul(size_of::<CapabilityEntry>())?;
        for text in [&self.host_id.0, &self.boot_id.0, &self.base_instance_id.0] {
            bytes = bytes.checked_add(text.capacity())?;
        }
        for entry in &self.entries {
            bytes = bytes.checked_add(
                entry
                    .active_sequences
                    .capacity()
                    .checked_mul(size_of::<u32>())?,
            )?;
            let s = &entry.inspection.scope;
            for text in [
                &entry.inspection.possession_id.0,
                &s.host_id.0,
                &s.boot_id.0,
                &s.base_instance_id.0,
                &s.plan_id.0,
                &s.active_play_id.0,
                &s.authority_grant_id.0,
                &s.authority_contract_id.0,
                &s.capability_id.0,
                &s.implementation_id.0,
                &s.operation_contract_id.0,
                &s.subject_kind.0,
                &s.resource_pool_id.0,
                &s.resource_generation_id.0,
                &s.envelope_id.0,
            ] {
                bytes = bytes.checked_add(text.capacity())?;
            }
        }
        Some(bytes)
    }
}
impl BaseOperationClaim {
    /// Actual identity backing only; excludes inline Self and allocator overhead.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = 0usize;
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
            bytes = bytes.checked_add(text.capacity())?;
        }
        Some(bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_table_capacity_counts_spare_ids_and_reserved_slots_without_issuance() {
        let mut host = HostId::from("host/capacity");
        host.0.reserve(93);
        let expected = host.0.capacity();
        let table = BaseCapabilityTable::new(
            host,
            "boot/capacity".into(),
            "base/capacity".into(),
            1,
            [9; 32],
            3,
        )
        .unwrap();
        assert_eq!(
            table.retained_backing_bytes(),
            Some(
                expected
                    + table.boot_id.0.capacity()
                    + table.base_instance_id.0.capacity()
                    + table.entries.capacity() * size_of::<CapabilityEntry>()
            )
        );
        assert_eq!(table.inspections().count(), 0);
    }
}
