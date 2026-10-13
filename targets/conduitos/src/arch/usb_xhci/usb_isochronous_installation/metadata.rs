//! Preparation-only accounting for the exact retained provider.
use super::*;

// Two preissued single-entry tables + claims have bounded identifier text.
// 16KiB each covers table entry/active-sequence storage, scope/claim strings and
// allocator rounding; both original tables are created with capacity one by Root.
pub(super) fn validate_preissued_metadata(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
) -> Result<(), UsbIsochronousCallRefusal> {
    let mut entries = table.inspections();
    let entry = entries.next().ok_or(UsbIsochronousCallRefusal::Binding)?;
    if entries.next().is_some()
        || entry.scope.maximum_in_flight != 1
        || table.maximum_capabilities() != 1
        || table.reserved_capability_slots() != 1
    {
        return Err(UsbIsochronousCallRefusal::Binding);
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
    // Root's arena uses 32-byte allocation units. Length does not bound
    // String capacity: charge original issuer, scope, claim and Vec allocations.
    let metadata_bytes = table
        .retained_storage_bytes(32)
        .and_then(|bytes| bytes.checked_add(claim.retained_storage_bytes(32)?));
    if strings.iter().any(|text| text.len() > 256)
        || strings.iter().map(|text| text.len()).sum::<usize>() > 4096
        || metadata_bytes.is_none_or(|bytes| bytes > 16384)
    {
        return Err(UsbIsochronousCallRefusal::Binding);
    }
    Ok(())
}
pub(super) fn admit_retained_resource(
    newly_retained: usize,
    admitted: u64,
) -> Result<usize, UsbIsochronousCallRefusal> {
    let charged =
        retained_resource_bytes(newly_retained).ok_or(UsbIsochronousCallRefusal::Binding)?;
    if charged as u64 > admitted {
        return Err(UsbIsochronousCallRefusal::Binding);
    }
    Ok(charged)
}
pub(super) fn retained_resource_bytes(newly_retained: usize) -> Option<usize> {
    newly_retained
        .checked_add(2 * 16384)? // preissued tables and their claims
        .checked_add(core::mem::size_of::<NativeUsbIsochronousHost>())?
        .checked_add(8192)? // rooted ring, packet buffers, input context and page padding
        .checked_add(4 * ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize) // ISO Flow budget, independently prepared in the kernel
}
