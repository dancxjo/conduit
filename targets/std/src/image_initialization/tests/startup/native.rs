//! Diagnostic SDL CPU ownership plus the actual no-Mask cue Body, not physical output proof.
use super::*;
use std::sync::{atomic::Ordering, Mutex};
static SDL: Mutex<()> = Mutex::new(());
#[test]
fn native_fixed_owner_survives_retained_started_cue_on_one_image_counter() {
    let _lock = SDL.lock().unwrap();
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window("same IMAGE cue diagnostic", 640, 480)
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut output =
        conduit_linux_display::Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    let (config, selected, _, _) = fixture::equipment(FakePlaybackBehavior::Success, 67108864);
    let source = fixture::source(67108864).replace("  limits =", r#"  base = {kind: "display/scanout", implementation: "display/linear-framebuffer@1", initialization: {backend: "KMSDRM", width: 640, height: 480}}
  need = {id: "display/main", class: "face/surface", slots: 1, bytes: 1228800}
  need = {id: "display/show", class: "face.resource/show@1", slots: 1, bytes: 1}
  need = {id: "display/memory", class: "conduit.resource/runtime-memory@1", slots: 8388608, bytes: 8388608}
  limits ="#);
    let catalog = conduit_workspace_make::catalog();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let image = super::super::image(&source);
    let host = StdHost::from_native_mask_diagnostic_with_playback(
        config,
        &image,
        &catalog,
        &checked,
        conduit_linear_framebuffer_make::CheckedNativeMaskInitialization::from_configuration(
            &checked,
        )
        .unwrap(),
        &mut output,
        None,
        &selected,
    )
    .unwrap();
    let token = output.initialized_display().unwrap();
    let pointer = host.ready_selected_native_mask().unwrap() as *const _;
    let fixed = host.ready_selected_native_mask().unwrap().fixed_backing();
    assert!(fixed.mask.managed_store_bytes > 0);
    let counter = host.kernel_resources.image_memory_observer();
    let baseline = counter.load(Ordering::Acquire);
    assert!(baseline >= fixed.image_heap_charge);
    // The next IMAGE is exactly one byte below the retained constructor charge.
    // Pressure must win before attempting to claim this already-owned output.
    let short_source = source.replace(
        "heap_arena_bytes: 67108864",
        &format!("heap_arena_bytes: {}", baseline - 1),
    );
    let short_checked = check_host_configuration(
        parse_host_configuration_conduit(&short_source).unwrap(),
        &catalog,
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let short_image = super::super::image(&short_source);
    let short = StdHost::from_native_mask_diagnostic_with_playback(
        crate::StdHostConfig {
            host_id: host.advertisement().host_id.clone(),
            boot_id: host.advertisement().boot_id.clone(),
            offer_generation: host.advertisement().offer_generation,
        },
        &short_image,
        &catalog,
        &short_checked,
        conduit_linear_framebuffer_make::CheckedNativeMaskInitialization::from_configuration(
            &short_checked,
        )
        .unwrap(),
        &mut output,
        None,
        &selected,
    );
    let refusal = short.err().expect("one-byte-short constructor admitted");
    assert!(
        refusal.contains("IMAGE") || refusal.contains("memory"),
        "{refusal}"
    );
    assert!(token.is_live());
    assert_eq!(counter.load(Ordering::Acquire), baseline);
    let (mut host, mut session) = fixture::session_for_host(host);
    let proposed = session.realization().unwrap().clone();
    let ad = host.advertisement().clone();
    let report = host
        .run_selected_image_body_plan_with_committed_startup_to(
            BodyRunRequest {
                wake: &proposed.wake,
                plan: &proposed.plan,
                control: &crate::RunControl::default(),
                keyboard: None,
            },
            &[],
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            None,
            |play, wake| {
                assert!(token.is_live());
                session
                    .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                    .unwrap();
                CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, play)
            },
        )
        .unwrap();
    assert_eq!(
        report.report().startup_chimes[0].disposition,
        Some(HostCallDisposition::Completed)
    );
    assert!(token.is_live());
    drop(report);
    assert_eq!(
        host.ready_selected_native_mask().unwrap() as *const _,
        pointer
    );
    assert_eq!(
        host.ready_selected_native_mask().unwrap().fixed_backing(),
        fixed
    );
    assert_eq!(counter.load(Ordering::Acquire), baseline);
    assert!(host.ready_selected_native_mask().unwrap().plan().is_none());
    drop(host);
    // This observer still owns the counter allocation itself. All native,
    // playback and report charges are gone; the final strong observer must
    // release the counter too, rather than hiding another retained owner.
    assert_eq!(
        counter.load(Ordering::Acquire),
        crate::kernel_preparation::StagedImageMemory::counter_backing_bytes()
    );
    let remaining = std::sync::Arc::downgrade(&counter);
    drop(counter);
    assert!(remaining.upgrade().is_none());
}
