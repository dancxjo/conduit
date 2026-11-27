//! Finite caller-owned selected-read metadata, independent of payload format.
use super::CapturedSnapshotSelection;
use conduit_linux_snapshot_store::{PreparedFileSnapshot, MAXIMUM_SNAPSHOT_BYTES};
const MAXIMUM_SELECTION_ID_BYTES: usize = 512;

pub(crate) fn validate_capture_selection(
    entry: &CapturedSnapshotSelection<'_>,
) -> Result<(), String> {
    let a = entry.selected.authority;
    let content = entry
        .selected
        .binding
        .content
        .as_ref()
        .ok_or("quantity history selection has no content contract")?;
    let grant = entry.original_grant;
    let local = entry.local_reference;
    if [
        entry.selected.host.as_str(),
        entry.selected.boot.as_str(),
        entry.selected.kind.as_str(),
        entry.selected.capability.as_str(),
        a.grant_id.as_str(),
        a.contract_id.as_str(),
        a.host_call_contract_id.as_str(),
        a.subject_kind.as_str(),
        a.host_id.as_str(),
        a.boot_id.as_str(),
        a.capability_id.as_str(),
        entry.selected.binding.class_id.as_str(),
        content.contract.content_profile.as_str(),
        entry.reference.content_profile.as_str(),
        entry.reference.access_class.as_str(),
        grant.grant_id.as_str(),
        grant.contract_id.as_str(),
        grant.host_call_contract_id.as_str(),
        grant.subject_kind.as_str(),
        grant.host_id.as_str(),
        grant.boot_id.as_str(),
        grant.capability_id.as_str(),
        local.content_profile.as_str(),
        local.access_class.as_str(),
        local.handle.as_str(),
        local.authority_contract.as_str(),
        local.authority_grant.as_str(),
    ]
    .iter()
    .any(|value| value.is_empty() || value.len() > MAXIMUM_SELECTION_ID_BYTES)
        || entry.reference.content_profile.as_str() != "text/json-utf8@1"
        || entry.reference.access_class.as_str() != "resource/snapshot@1"
        || !(1..=MAXIMUM_SNAPSHOT_BYTES as u64).contains(&entry.reference.extent.bytes)
        || entry.reference.extent.items != Some(1)
        || entry.reference.lifetime.expires_at.is_some()
    {
        return Err(
            "quantity history selected metadata or snapshot exceeds its admitted profile".into(),
        );
    }

    Ok(())
}
/// One file operation's inline read buffers, bounded cloned authority/reference
/// metadata and encoding scratch. Caller-owned selections remain separate.
pub(crate) fn selected_capture_preparation_backing_bound() -> Result<usize, String> {
    let record_metadata = 16 * MAXIMUM_SELECTION_ID_BYTES + 512;
    core::mem::size_of::<PreparedFileSnapshot<'_>>()
        .checked_add(record_metadata)
        .and_then(|bytes| bytes.checked_add(MAXIMUM_SNAPSHOT_BYTES))
        .ok_or_else(|| "selected snapshot preparation backing overflow".into())
}
