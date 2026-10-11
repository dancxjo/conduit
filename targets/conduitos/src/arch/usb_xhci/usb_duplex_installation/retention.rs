//! Finite retention admission before any independently routed bulk publication.
use super::*;

pub(super) fn validate_metadata(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
) -> Result<(), UsbNativeProtocolRefusal> {
    let mut entries = table.inspections();
    let entry = entries.next().ok_or(read_binding())?;
    if entries.next().is_some()
        || entry.scope.maximum_in_flight != 1
        || table.maximum_capabilities() != 1
        || table.reserved_capability_slots() != 1
    {
        return Err(read_binding());
    }
    let scope = &entry.scope;
    let strings = [
        scope.host_id.as_str(),
        scope.boot_id.as_str(),
        scope.base_instance_id.as_str(),
        scope.plan_id.as_str(),
        scope.active_play_id.as_str(),
        scope.authority_grant_id.as_str(),
        scope.authority_contract_id.as_str(),
        scope.capability_id.as_str(),
        scope.implementation_id.as_str(),
        scope.operation_contract_id.as_str(),
        scope.subject_kind.as_str(),
        scope.resource_pool_id.as_str(),
        &scope.resource_generation_id.0,
        scope.envelope_id.as_str(),
        claim.host_id.as_str(),
        claim.boot_id.as_str(),
        claim.base_instance_id.as_str(),
        claim.plan_id.as_str(),
        claim.active_play_id.as_str(),
        claim.implementation_id.as_str(),
        claim.operation_contract_id.as_str(),
        claim.subject_kind.as_str(),
        claim.resource_pool_id.as_str(),
        &claim.resource_generation_id.0,
        claim.envelope_id.as_str(),
    ];
    if strings.iter().any(|text| text.len() > 256)
        || strings.iter().map(|text| text.len()).sum::<usize>() > 4096
    {
        return Err(read_binding());
    }
    Ok(())
}

pub(super) fn retained_bytes(newly_retained: usize) -> Option<usize> {
    newly_retained
        .checked_add(3 * 16384)? // preissued tables/claims, metadata bounded below
        .checked_add(core::mem::size_of::<NativeUsbDuplexHost>())?
        .checked_add(8192)? // both rooted DMA rings/buffers, context and page padding
        .checked_add(6 * 4096) // independently prepared control/read/write request/result Flows
}

pub(super) fn admit(
    newly_retained: usize,
    admitted: [u64; 2],
) -> Result<usize, UsbNativeProtocolRefusal> {
    let charge = retained_bytes(newly_retained).ok_or_else(read_binding)?;
    // Both grants cover an equal, independently bounded share; neither borrows
    // capacity from its sibling or from the control DMA resource.
    let each = charge.checked_add(1).ok_or_else(read_binding)? / 2;
    if admitted.into_iter().any(|bytes| bytes < each as u64) {
        return Err(read_binding());
    }
    Ok(charge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb_base::{endpoint_read_contract::*, endpoint_read_owner::EndpointReadCallOwner};
    #[allow(dead_code)]
    #[path = "../../../usb_base/endpoint_read_owner/tests/fixture.rs"]
    mod read_fixture;

    #[test]
    fn one_live_entry_does_not_hide_an_oversized_retained_table() {
        for capacity in [1, 4] {
            let scope = read_fixture::scope();
            let mut table = BaseCapabilityTable::new(
                scope.host_id,
                scope.boot_id,
                scope.base_instance_id,
                4,
                [77; 32],
                capacity,
            )
            .unwrap();
            table.issue(read_fixture::request()).unwrap();
            assert_eq!(table.inspections().count(), 1);
            assert_eq!(
                validate_metadata(&table, &read_fixture::claim()).is_ok(),
                capacity == 1
            );
        }
    }
    #[test]
    fn charge_contains_inline_host_and_actual_heap_terminals_without_double_counting() {
        let terminals = 2 * (minimum_endpoint_resource_bytes() as usize - 3072);
        let charge = retained_bytes(terminals).unwrap();
        assert_eq!(
            charge,
            terminals + 3 * 16384 + core::mem::size_of::<NativeUsbDuplexHost>() + 8192 + 6 * 4096
        );
        assert!(charge > terminals + core::mem::size_of::<NativeUsbDuplexHost>());
        assert_eq!(retained_bytes(usize::MAX), None);
    }
    #[test]
    fn both_independent_grants_must_cover_their_exact_retained_share() {
        let charge = retained_bytes(12345).unwrap();
        let each = charge.div_ceil(2) as u64;
        assert_eq!(admit(12345, [each, each]).unwrap(), charge);
        assert!(admit(12345, [each - 1, u64::MAX]).is_err());
        assert!(admit(12345, [u64::MAX, each - 1]).is_err());
        assert!(admit(usize::MAX, [u64::MAX, u64::MAX]).is_err());
    }
}
