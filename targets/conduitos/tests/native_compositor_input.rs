use conduit_presentation::LayoutRect;
use conduitos::native_compositor::{DamageRect, InputRoute, NativeCompositorError};

#[path = "common/native_compositor.rs"]
mod support;
use support::MemoryDisplay;
#[path = "common/native_compositor_fixture.rs"]
mod fixture_support;
use fixture_support::{MAIN, TOP, delivered, fixture, update, update_with_scene};

#[test]
fn pointer_routing_obeys_z_geometry_visibility_removal_and_empty_space() {
    let (presentation, plan, main, top, mut compositor) = fixture(1);
    update(&mut compositor, &presentation, &plan, &main, MAIN);
    update(&mut compositor, &presentation, &plan, &top, TOP);

    let route = delivered(compositor.route_pointer(7, 4, false).unwrap());
    assert_eq!(route.surface_id, TOP);
    assert_eq!((route.local_x, route.local_y), (2, 2));
    let main_identity = main.manifestation_id.clone();
    let top_identity = top.manifestation_id.clone();

    compositor
        .place_surface(
            MAIN,
            LayoutRect {
                x: 1,
                y: 1,
                width: 12,
                height: 8,
            },
            2,
        )
        .unwrap();
    let route = delivered(compositor.route_pointer(7, 4, false).unwrap());
    assert_eq!(route.surface_id, MAIN);
    assert_eq!(route.manifestation_id, main_identity);
    assert_eq!(top.manifestation_id, top_identity);

    compositor.set_surface_visible(MAIN, false).unwrap();
    assert_eq!(
        delivered(compositor.route_pointer(7, 4, false).unwrap()).surface_id,
        TOP
    );
    compositor.remove_surface(TOP).unwrap();
    assert_eq!(
        compositor.route_pointer(7, 4, false).unwrap(),
        InputRoute::NoTarget
    );
    assert_eq!(
        compositor.route_pointer(31, 15, false).unwrap(),
        InputRoute::NoTarget
    );
}

#[test]
fn activation_focuses_exact_surface_and_keyboard_never_falls_through() {
    let (presentation, plan, main, top, mut compositor) = fixture(1);
    update(&mut compositor, &presentation, &plan, &main, MAIN);
    update(&mut compositor, &presentation, &plan, &top, TOP);

    let route = delivered(compositor.route_pointer(7, 4, true).unwrap());
    assert_eq!(route.surface_id, TOP);
    let keyboard = delivered(compositor.route_keyboard(&top.manifestation_id).unwrap());
    assert_eq!(keyboard.surface_id, TOP);
    assert_eq!(keyboard.manifestation_id, top.manifestation_id);

    compositor.set_surface_visible(TOP, false).unwrap();
    assert_eq!(
        compositor.route_keyboard(&top.manifestation_id).unwrap(),
        InputRoute::NoTarget
    );
    assert_ne!(compositor.focused_surface(), Some(MAIN));
    compositor.set_surface_visible(TOP, true).unwrap();
    compositor.remove_surface(TOP).unwrap();
    assert_eq!(
        compositor.route_keyboard(&top.manifestation_id).unwrap(),
        InputRoute::NoTarget
    );
}

#[test]
fn stale_manifestation_routes_refuse_distinctly_from_no_target() {
    let (presentation, plan, main, _top, mut compositor) = fixture(1);
    update(&mut compositor, &presentation, &plan, &main, MAIN);
    let old_route = delivered(compositor.route_pointer(2, 2, true).unwrap());

    let (next_presentation, next_plan, next_main, _, _) = fixture(2);
    update(
        &mut compositor,
        &next_presentation,
        &next_plan,
        &next_main,
        MAIN,
    );
    assert_eq!(
        compositor.validate_pointer_route(&old_route),
        Err(NativeCompositorError::StaleSurfaceBinding)
    );
    assert_eq!(
        compositor.route_keyboard(&main.manifestation_id),
        Err(NativeCompositorError::StaleSurfaceBinding)
    );
    let keyboard = delivered(
        compositor
            .route_keyboard(&next_main.manifestation_id)
            .unwrap(),
    );
    assert_eq!(keyboard.manifestation_id, next_main.manifestation_id);
}

#[test]
fn resizing_invalidates_pixels_and_input_until_a_fresh_manifestation_revision() {
    let (presentation, plan, main, _top, mut compositor) = fixture(1);
    update(&mut compositor, &presentation, &plan, &main, MAIN);
    assert!(matches!(
        compositor.route_pointer(2, 2, true).unwrap(),
        InputRoute::Delivered(_)
    ));

    compositor
        .place_surface(
            MAIN,
            LayoutRect {
                x: 1,
                y: 1,
                width: 8,
                height: 6,
            },
            0,
        )
        .unwrap();
    assert_eq!(
        compositor.route_pointer(2, 2, false).unwrap(),
        InputRoute::NoTarget
    );
    assert_eq!(
        compositor.route_keyboard(&main.manifestation_id).unwrap(),
        InputRoute::NoTarget
    );
    assert_eq!(compositor.receipts().count(), 0);
    assert_eq!(
        compositor.update_surface(
            &presentation,
            &main,
            &plan,
            MAIN,
            &conduit_core::HostBaseId::from(fixture_support::BASE),
            &conduit_presentation::GraphicsScene::empty(),
        ),
        Err(NativeCompositorError::StaleSurfaceRevision)
    );

    let (next_presentation, next_plan, next_main, _, _) = fixture(2);
    update(
        &mut compositor,
        &next_presentation,
        &next_plan,
        &next_main,
        MAIN,
    );
    let routed = delivered(compositor.route_pointer(2, 2, true).unwrap());
    assert_eq!(routed.manifestation_id, next_main.manifestation_id);
}

#[test]
fn damage_is_bounded_partial_clean_clipped_and_retryable() {
    let (presentation, plan, main, _top, mut compositor) = fixture(1);
    update(&mut compositor, &presentation, &plan, &main, MAIN);
    let mut display = MemoryDisplay::new();
    let initial = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(initial.damage_count, 1);
    assert_eq!(
        initial.damage_rects[0],
        DamageRect {
            x: 1,
            y: 1,
            width: 12,
            height: 8
        }
    );
    assert_eq!(initial.pixels_written, 96);
    display.reset_writes();
    let clean = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(
        (clean.damage_count, clean.pixels_written, display.writes),
        (0, 0, 0)
    );

    let (next_presentation, next_plan, next_main, _, _) = fixture(2);
    update(
        &mut compositor,
        &next_presentation,
        &next_plan,
        &next_main,
        MAIN,
    );
    display.fail_after = Some(5);
    assert_eq!(
        compositor.compose_frame(&mut display),
        Err(NativeCompositorError::Display(
            conduitos::display::DisplayError::Lost
        ))
    );
    display.fail_after = None;
    display.reset_writes();
    let retried = compositor.compose_frame(&mut display).unwrap();
    assert_eq!((retried.pixels_written, display.writes), (96, 96));

    compositor
        .place_surface(
            MAIN,
            LayoutRect {
                x: -3,
                y: -2,
                width: 12,
                height: 8,
            },
            0,
        )
        .unwrap();
    display.reset_writes();
    let clipped = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(
        clipped.damage_rects[0],
        DamageRect {
            x: 0,
            y: 0,
            width: 13,
            height: 9
        }
    );
    assert_eq!(clipped.pixels_written, 117);
}

#[test]
fn surface_operations_damage_only_changed_output_regions() {
    let (presentation, plan, main, top, mut compositor) = fixture(1);
    update(&mut compositor, &presentation, &plan, &main, MAIN);
    update(&mut compositor, &presentation, &plan, &top, TOP);
    let mut display = MemoryDisplay::new();
    compositor.compose_frame(&mut display).unwrap();

    compositor
        .place_surface(
            TOP,
            LayoutRect {
                x: 20,
                y: 1,
                width: 10,
                height: 7,
            },
            1,
        )
        .unwrap();
    compositor.compose_frame(&mut display).unwrap();
    let (next_presentation, next_plan, _, next_top, _) = fixture(2);
    update(
        &mut compositor,
        &next_presentation,
        &next_plan,
        &next_top,
        TOP,
    );
    display.reset_writes();
    let isolated_update = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(isolated_update.pixels_written, 70);
    assert_eq!(display.writes, 70);
    compositor
        .place_surface(
            TOP,
            LayoutRect {
                x: 5,
                y: 2,
                width: 10,
                height: 7,
            },
            1,
        )
        .unwrap();
    compositor.compose_frame(&mut display).unwrap();

    compositor
        .place_surface(
            MAIN,
            LayoutRect {
                x: 18,
                y: 1,
                width: 12,
                height: 8,
            },
            0,
        )
        .unwrap();
    display.reset_writes();
    let moved = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(moved.damage_count, 2);
    assert_eq!(moved.pixels_written, 192);
    assert_eq!(display.pixels[1 + 32], 0);

    compositor.set_surface_visible(TOP, false).unwrap();
    display.reset_writes();
    let hidden = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(hidden.pixels_written, 70);
    compositor.set_surface_visible(TOP, true).unwrap();
    assert_eq!(
        compositor
            .compose_frame(&mut display)
            .unwrap()
            .pixels_written,
        70
    );
    compositor.remove_surface(TOP).unwrap();
    assert_eq!(
        compositor
            .compose_frame(&mut display)
            .unwrap()
            .pixels_written,
        70
    );
}

#[test]
fn z_changes_damage_exact_overlap_and_fragmentation_falls_back() {
    let (presentation, plan, main, top, mut compositor) = fixture(1);
    update(&mut compositor, &presentation, &plan, &main, MAIN);
    update(&mut compositor, &presentation, &plan, &top, TOP);
    let mut display = MemoryDisplay::new();
    compositor.compose_frame(&mut display).unwrap();

    compositor
        .place_surface(
            MAIN,
            LayoutRect {
                x: 1,
                y: 1,
                width: 12,
                height: 8,
            },
            2,
        )
        .unwrap();
    let raised = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(raised.damage_count, 1);
    assert_eq!(
        raised.damage_rects[0],
        DamageRect {
            x: 5,
            y: 2,
            width: 8,
            height: 7
        }
    );
    assert_eq!(raised.pixels_written, 56);

    compositor
        .place_surface(
            MAIN,
            LayoutRect {
                x: 0,
                y: 12,
                width: 1,
                height: 1,
            },
            2,
        )
        .unwrap();
    compositor.compose_frame(&mut display).unwrap();
    for step in 1..=18 {
        compositor
            .place_surface(
                MAIN,
                LayoutRect {
                    x: step * 2,
                    y: 12,
                    width: 1,
                    height: 1,
                },
                2,
            )
            .unwrap();
    }
    let fallback = compositor.compose_frame(&mut display).unwrap();
    assert!(fallback.conservative_fallback);
    assert_eq!(fallback.damage_count, 1);
}

#[test]
fn patchbay_presentation_revision_recomposes_only_its_native_surface() {
    use conduit_tour_model::TourWorkspacePhase;

    let (presentation, plan, _main, top, mut compositor) = fixture(1);
    let initial_scene =
        conduitos::tour_workspace::scene(10, 7, 1, TourWorkspacePhase::LessonReady).unwrap();
    update_with_scene(
        &mut compositor,
        &presentation,
        &plan,
        &top,
        TOP,
        &initial_scene,
    );
    let mut display = MemoryDisplay::new();
    compositor.compose_frame(&mut display).unwrap();

    let (next_presentation, next_plan, _, next_top, _) = fixture(2);
    let patchbay_scene =
        conduitos::tour_workspace::scene(10, 7, 2, TourWorkspacePhase::PatchbayOpen).unwrap();
    update_with_scene(
        &mut compositor,
        &next_presentation,
        &next_plan,
        &next_top,
        TOP,
        &patchbay_scene,
    );
    display.reset_writes();
    let frame = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(frame.damage_count, 1);
    assert_eq!(frame.pixels_written, 70);
    assert_eq!(display.writes, 70);
    assert!(frame.pixels_written < display.format.width * display.format.height);
}
