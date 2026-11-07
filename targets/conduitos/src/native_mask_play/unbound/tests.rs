use super::*;

fn planned() -> PlannedMaskPlot {
    crate::mask_control::prepare_stage(
        crate::mask_control::Adapter::Native,
        &"host/unbound-mask".into(),
        &"boot/unbound-mask".into(),
        7,
        None,
    )
    .unwrap()
    .planned_mask
}

#[test]
fn actual_stores_precede_plan_and_kernel_binds_in_the_original_box() {
    let profile = NativeMaskStorageProfile::Standard;
    let required = RetainedNativeMaskStorage::required_store_bytes(profile).unwrap();
    assert!(matches!(
        UnboundNativeMaskStorage::prepare(profile, required - 1),
        Err(NativeMaskPlayError::Pressure)
    ));
    let backing = UnboundNativeMaskStorage::prepare(profile, required).unwrap();
    let address = backing.backing_identity();
    let receipt = backing.backing();
    assert_eq!(receipt.managed_store_bytes, required);
    assert!(receipt.optional_output_storage_bytes > 0);
    // Only now does checked Plan/lowering preparation take place.
    let mask = planned();
    let retained = backing.bind(&mask).unwrap();
    assert_eq!(retained.backing_identity(), address);
    assert_eq!(retained.backing(), receipt);
}

#[test]
fn binding_refuses_invalid_plan_and_does_not_enlarge_or_relabel_profile() {
    let profile = NativeMaskStorageProfile::Standard;
    let required = RetainedNativeMaskStorage::required_store_bytes(profile).unwrap();
    let mut invalid = planned();
    invalid.plan.plan_id = "untrusted-relabel".into();
    assert!(matches!(
        UnboundNativeMaskStorage::prepare(profile, required)
            .unwrap()
            .bind(&invalid),
        Err(NativeMaskPlayError::Plan)
    ));
    let collection_required =
        RetainedNativeMaskStorage::required_store_bytes(NativeMaskStorageProfile::Collection)
            .unwrap();
    assert!(matches!(
        UnboundNativeMaskStorage::prepare(
            NativeMaskStorageProfile::Collection,
            collection_required
        )
        .unwrap()
        .bind(&planned()),
        Err(NativeMaskPlayError::Plan)
    ));
    // Unwritten scheduler allocation and actual stores can retire before a Plan
    // exists; MaybeUninit drops no fabricated driver/kernel or live Play.
    drop(UnboundNativeMaskStorage::prepare(profile, required).unwrap());
}
