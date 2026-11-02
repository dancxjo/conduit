//! CPU/SDL diagnostics only: these do not establish KMSDRM or physical HDMI.
use super::*;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs,
};
use conduit_linear_framebuffer_make::CheckedNativeMaskInitialization;
use conduit_linux_display::Scanout;

fn selected(
    memory: u32,
) -> (
    conduit_host_make::CheckedHostConfiguration,
    conduit_host_make::HostImage,
) {
    let source = include_str!("../../profiles/raspberry-pi-3-tick-workset.host.conduit")
        .replace("  capability =", &format!(r#"  base = {{kind: "display/scanout", implementation: "display/linear-framebuffer@1", initialization: {{backend: "KMSDRM", width: 640, height: 480}}}}
  need = {{id: "display/main", class: "face/surface", slots: 1, bytes: 1228800}}
  need = {{id: "display/show", class: "face.resource/show@1", slots: 1, bytes: 1}}
  need = {{id: "display/memory", class: "conduit.resource/runtime-memory@1", slots: {memory}, bytes: {memory}}}
  capability ="#));
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "diagnostic/selected-native-mask".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    (checked, image)
}
fn config() -> crate::StdHostConfig {
    crate::StdHostConfig {
        host_id: "diagnostic/native-mask".into(),
        boot_id: "diagnostic/boot".into(),
        offer_generation: OfferGeneration(7),
    }
}
#[test]
fn actual_fixed_stores_bind_exact_checked_image_and_parent_ledger_without_play() {
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window("native Mask CPU diagnostic", 640, 480)
        .hidden()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut output = Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    let (checked, image) = selected(8 * 1024 * 1024);
    let catalog = conduit_workspace_make::catalog();
    assert!(StdHost::from_selected_timer_and_native_mask_image(
        config(),
        &image,
        &catalog,
        &checked,
        CheckedNativeMaskInitialization::from_configuration(&checked).unwrap(),
        &mut output
    )
    .err()
    .unwrap()
    .contains("KMSDRM"));
    let (small, small_image) = selected(1);
    assert!(StdHost::from_native_mask_diagnostic(
        config(),
        &small_image,
        &catalog,
        &small,
        CheckedNativeMaskInitialization::from_configuration(&small).unwrap(),
        &mut output
    )
    .is_err());
    let mut host = StdHost::from_native_mask_diagnostic(
        config(),
        &image,
        &catalog,
        &checked,
        CheckedNativeMaskInitialization::from_configuration(&checked).unwrap(),
        &mut output,
    )
    .unwrap();
    let observed = host.kernel_resources.image_memory_observer();
    let fixed = host.ready_selected_native_mask().unwrap().fixed_backing();
    assert!(fixed.mask.managed_store_bytes > 0);
    assert_eq!(
        observed.load(std::sync::atomic::Ordering::Acquire),
        fixed.image_heap_charge
    );
    assert_eq!(host.base_registry.entries().len(), 2);
    let mut mask = host
        .selected_scanout
        .as_mut()
        .unwrap()
        .native_mask
        .take()
        .unwrap();
    assert!(mask.plan().is_none());
    let (encounter, route, face) = super::encounter_fixture::encounter(&host.advertisement, 1);
    let planned = route.planned_mask.clone();
    mask.bind(&mut host, &planned).unwrap();
    assert_eq!(mask.plan(), Some(&planned));
    let bound_bytes = observed.load(std::sync::atomic::Ordering::Acquire);
    assert!(bound_bytes > fixed.image_heap_charge);
    // Rebinding the exact immutable Plan allocates no replacement stores/lease.
    mask.bind(&mut host, &planned).unwrap();
    assert_eq!(
        observed.load(std::sync::atomic::Ordering::Acquire),
        bound_bytes
    );
    assert!(
        matches!(&mask.stores, Stores::Bound { renderer, .. } if renderer.session_absent_for_test())
    );
    host.selected_scanout.as_mut().unwrap().native_mask = Some(mask);
    assert!(host.ready_selected_native_mask().is_some());
    let frame = host
        .render_selected_native_mask(
            &encounter,
            &route,
            &face,
            1,
            "front/native-mask",
            &mut output,
        )
        .unwrap();
    assert_eq!(frame.face_revision, 1);
    assert!(frame.scanout_frame_sequence > 0);
    assert!(
        encounter.calls.get() >= 3,
        "authority remains current before prep/Step/effect"
    );
    let actual_show = host.selected_native_mask_show().unwrap();
    assert_eq!(
        actual_show.show.lifecycle,
        conduit_face::ManifestationLifecycle::Available
    );
    assert_eq!(actual_show.face_id, face.identity);
    let during = observed.load(std::sync::atomic::Ordering::Acquire);
    assert!(during > bound_bytes);
    assert!(
        host.render_selected_native_mask(
            &encounter,
            &route,
            &face,
            2,
            "front/native-mask",
            &mut output
        )
        .is_err(),
        "pending evidence is not silently cleared"
    );
    let mut retained = host.take_selected_native_mask_evidence().unwrap();
    let owner = retained.as_ref().unwrap();
    assert!(owner.evidence().receipt.scanout_pixels_written > 0);
    assert_eq!(owner.possession().lifecycle, "revoked-at-play-end");
    assert_eq!(owner.composition().face_id, face.identity);
    assert_eq!(
        observed.load(std::sync::atomic::Ordering::Acquire),
        during,
        "transferring evidence is not releasing its managed custody"
    );
    let mut foreign = StdHost::new();
    assert!(foreign
        .retire_selected_native_mask_evidence(&mut retained)
        .is_err());
    assert!(
        retained.is_some(),
        "scope refusal preserves complete evidence"
    );
    host.retire_selected_native_mask_evidence(&mut retained)
        .unwrap();
    assert!(retained.is_none());
    assert_eq!(
        observed.load(std::sync::atomic::Ordering::Acquire),
        bound_bytes
    );
    let (next, next_route, next_face) = super::encounter_fixture::encounter(&host.advertisement, 2);
    assert_eq!(next_route.planned_mask.plan, route.planned_mask.plan);
    host.render_selected_native_mask(
        &next,
        &next_route,
        &next_face,
        2,
        "front/native-mask",
        &mut output,
    )
    .unwrap();
    let mut retained = host.take_selected_native_mask_evidence().unwrap();
    let mut foreign = StdHost::new();
    assert!(foreign
        .retire_selected_native_mask_evidence(&mut retained)
        .is_err());
    assert!(
        retained.is_some(),
        "scope refusal preserves complete evidence"
    );
    host.retire_selected_native_mask_evidence(&mut retained)
        .unwrap();
    assert!(retained.is_none());
    assert_eq!(
        observed.load(std::sync::atomic::Ordering::Acquire),
        bound_bytes
    );
    drop(host);
    assert_eq!(observed.load(std::sync::atomic::Ordering::Acquire), 0);
}
