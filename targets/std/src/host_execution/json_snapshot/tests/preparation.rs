//! Allocation-profile qualification of the independently selected provider.
use super::*;

#[test]
fn admitted_preparation_profile_covers_actual_selected_provider_allocation() {
    let root = tempfile::tempdir().unwrap();
    let boot = "b".repeat(conduit_std_offers::JSON_SNAPSHOT_MAXIMUM_ID_BYTES);
    let (_, fragment, selected) = fixture(root.path(), &boot, Mode::Publish);
    let placement = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND)
        .unwrap();
    let probe = crate::allocation_probe::begin();
    let operation = selected.prepare_operation(placement).unwrap();
    // Match the actual Root Host's additional independently retained clone.
    let authority = placement.authority[0].clone();
    let allocation_bytes = probe.finish_bytes();
    // Four Host fields: provider, authority, numeric node and direction. Bound
    // each alignment gap independently, plus the enclosing Option's tag/rounding.
    // This does not depend on private JsonSnapshotHost layout visibility.
    let alignment = core::mem::align_of::<(
        crate::json_snapshot_file::PreparedJsonSnapshot<'_>,
        AuthorityBinding,
        conduit_kernel::NodeId,
    )>();
    let inline = core::mem::size_of_val(&operation)
        .checked_add(core::mem::size_of_val(&authority))
        .unwrap()
        .checked_add(core::mem::size_of::<conduit_kernel::NodeId>())
        .unwrap()
        .checked_add(core::mem::size_of::<bool>())
        .unwrap()
        .checked_add(5 * alignment)
        .unwrap();
    let charged = allocation_bytes.checked_add(inline).unwrap();
    eprintln!("selected JSON preparation allocations={allocation_bytes} inlineBound={inline} charged={charged} profile={} hostBytes={} bootBytes={} grantBytes={}",
        conduit_std_offers::JSON_SNAPSHOT_PREPARATION_BYTES,
        authority.host_id.as_str().len(), authority.boot_id.as_str().len(), authority.grant_id.as_str().len());
    assert!(
        charged <= conduit_std_offers::JSON_SNAPSHOT_PREPARATION_BYTES as usize,
        "provider/Host preparation requested {charged} bytes; admitted profile={}",
        conduit_std_offers::JSON_SNAPSHOT_PREPARATION_BYTES
    );
}
