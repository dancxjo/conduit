//! Preparing an installed Mask must preserve the owner's captured Body Plan.
use super::*;

#[test]
fn installed_mask_preparation_preserves_admitted_body_plan_and_refuses_missing_routes() {
    let (body, wake, base) = body_plan_basis();
    let face = specimen_face(&face_specimens()[0], body.clone(), 1);
    let host = HostId::from("host/browser");
    let boot = BootId::from("boot/browser");
    assert!(BrowserMaskRuntime::prepare_in_body_plan(
        body.clone(),
        host.clone(),
        boot.clone(),
        face.clone(),
        wake.clone(),
        base.clone(),
        8,
    )
    .is_err());
    // The existing recipe supplies a sealed fixture. The installed entrance
    // must use that exact seal, not repeat the recipe or silently add routes.
    let (recipe, _) = BrowserMaskRuntime::prepare(
        body.clone(),
        host.clone(),
        boot.clone(),
        face.clone(),
        wake.clone(),
        base,
    )
    .unwrap();
    let admitted = recipe.observation().body_plan;
    let (mut installed, effect) = BrowserMaskRuntime::prepare_in_body_plan(
        body.clone(),
        host.clone(),
        boot.clone(),
        face.clone(),
        wake.clone(),
        admitted.clone(),
        9,
    )
    .unwrap();
    assert!(installed.reserved_value_storage_bytes() >= 3 * MASK_BYTES as usize);
    assert!(installed.reserved_value_storage_bytes() < 4 * MASK_BYTES as usize);
    assert_eq!(installed.observation().body_plan, admitted);
    assert_eq!(installed.observation().face, face);
    assert_eq!(
        installed.observation().mask_show.show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    installed.acknowledge(&acknowledgement(&effect)).unwrap();
    let used = conduit_kernel::ValueStorage::used_items(installed.scheduler.values());
    for _ in 0..8 {
        assert!(installed.acknowledge(&acknowledgement(&effect)).is_err());
    }
    assert_eq!(
        conduit_kernel::ValueStorage::used_items(installed.scheduler.values()),
        used
    );
    assert_eq!(installed.observation().body_plan, admitted);
    assert_eq!(
        installed.observation().mask_show.show.lifecycle,
        ManifestationLifecycle::Available
    );
    assert!(BrowserMaskRuntime::prepare_in_body_plan(
        body_id(),
        host,
        boot,
        face,
        wake,
        admitted,
        10,
    )
    .is_err());
}
