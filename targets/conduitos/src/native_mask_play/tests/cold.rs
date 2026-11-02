use super::*;

#[test]
fn cold_invocation_owns_real_values_and_only_prepared_show_before_activation() {
    let (diagnostic, _) = fixture_scanout(1);
    let face = diagnostic.renderer_request().face().clone();
    drop(diagnostic);
    let planned = crate::mask_control::prepare_stage(
        crate::mask_control::Adapter::Native,
        &"host/native-mask".into(),
        &"boot/native-mask".into(),
        1,
        None,
    )
    .unwrap()
    .planned_mask;
    let profile = conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard;
    let maximum = RetainedNativeMaskStorage::required_store_bytes(profile).unwrap();
    let storage = UnboundNativeMaskStorage::prepare(profile, maximum)
        .unwrap()
        .bind(&planned)
        .unwrap();
    let mut cold = PreparedNativeMaskPlay::prepare_cold_with_storage(
        &planned,
        &face,
        1,
        "front/native-mask",
        "surface/native-mask",
        "display/native-mask".into(),
        profile,
        storage,
    )
    .unwrap();
    let before = cold.retained_metadata_bytes().unwrap();
    let bytes = &mut cold.parts.as_mut().unwrap().face_bytes;
    let previous = bytes.capacity();
    bytes.reserve(127);
    let added = bytes.capacity() - previous;
    assert_eq!(cold.retained_metadata_bytes(), Some(before + added));
    assert!(
        cold.idle_for_test(),
        "cold preparation performs no kernel Step or Fore input"
    );
    assert_eq!(
        cold.renderer_request()
            .unwrap()
            .prepared_show()
            .show
            .lifecycle,
        conduit_face::ManifestationLifecycle::Prepared
    );
    let pending = cold.start().unwrap();
    assert_eq!(
        pending.renderer_request().prepared_show().show.lifecycle,
        conduit_face::ManifestationLifecycle::Prepared,
        "only actual scanout acknowledgment can expose Available"
    );
    pending.cancel().unwrap();
}
