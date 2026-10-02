#[path = "common/native_compositor.rs"]
mod display;
#[allow(dead_code)]
#[path = "common/native_compositor_fixture.rs"]
mod fixture;

use conduit_core::PlanId;
use conduit_presentation::LayoutRect;
use display::MemoryDisplay;
use fixture::{MAIN, TOP, fixture, update};

#[test]
fn only_successful_scanout_mints_exact_revision_acknowledgement() {
    let (face, plan, main, _, mut compositor) = fixture(1);
    update(&mut compositor, &face, &plan, &main, MAIN);
    let staged = compositor.receipts().next().unwrap().clone();
    let allocated = compositor.allocated_pixels();
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
    let mut display = MemoryDisplay::new();
    let frame = compositor.compose_frame(&mut display).unwrap();
    let ack = compositor.scanout_acknowledgement(&staged, 1).unwrap();
    assert_eq!(ack.composition(), &staged);
    assert_eq!(ack.presentation_revision(), 1);
    assert_eq!(ack.frame_sequence(), frame.frame_sequence);
    assert_eq!(ack.pixels_written(), 12 * 8);
    assert_eq!(compositor.allocated_pixels(), allocated);
    assert!(compositor.scanout_acknowledgement(&staged, 2).is_none());
    let mut wrong = staged.clone();
    wrong.plan_id = PlanId::from("wrong/plan");
    assert!(compositor.scanout_acknowledgement(&wrong, 1).is_none());
    wrong = staged.clone();
    wrong.active_play_id = conduit_core::ActivePlayId::from("wrong/play");
    assert!(compositor.scanout_acknowledgement(&wrong, 1).is_none());
    // A no-op frame cannot manufacture another effect receipt.
    compositor.compose_frame(&mut display).unwrap();
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
}

#[test]
fn hidden_and_fully_occluded_surfaces_have_no_acknowledgement() {
    let (face, plan, main, top, mut compositor) = fixture(1);
    compositor
        .place_surface(
            TOP,
            LayoutRect {
                x: 1,
                y: 1,
                width: 12,
                height: 8,
            },
            1,
        )
        .unwrap();
    update(&mut compositor, &face, &plan, &main, MAIN);
    update(&mut compositor, &face, &plan, &top, TOP);
    let receipts: Vec<_> = compositor.receipts().cloned().collect();
    let mut display = MemoryDisplay::new();
    compositor.compose_frame(&mut display).unwrap();
    assert!(
        compositor
            .scanout_acknowledgement(&receipts[0], 1)
            .is_none()
    );
    assert!(
        compositor
            .scanout_acknowledgement(&receipts[1], 1)
            .is_some()
    );
    compositor.set_surface_visible(TOP, false).unwrap();
    assert!(
        compositor
            .scanout_acknowledgement(&receipts[1], 1)
            .is_none()
    );
    compositor.compose_frame(&mut display).unwrap();
    assert!(
        compositor
            .scanout_acknowledgement(&receipts[0], 1)
            .is_some()
    );
    assert!(
        compositor
            .scanout_acknowledgement(&receipts[1], 1)
            .is_none()
    );
}

#[test]
fn superseded_revision_cannot_borrow_new_revision_scanout() {
    let (face, plan, main, _, mut compositor) = fixture(1);
    update(&mut compositor, &face, &plan, &main, MAIN);
    let old = compositor.receipts().next().unwrap().clone();
    let mut display = MemoryDisplay::new();
    compositor.compose_frame(&mut display).unwrap();
    let (next, next_plan, next_main, _, _) = fixture(2);
    update(&mut compositor, &next, &next_plan, &next_main, MAIN);
    assert!(compositor.scanout_acknowledgement(&old, 1).is_none());
    let current = compositor.receipts().next().unwrap().clone();
    compositor.compose_frame(&mut display).unwrap();
    assert!(compositor.scanout_acknowledgement(&old, 1).is_none());
    assert!(compositor.scanout_acknowledgement(&current, 2).is_some());
}

#[test]
fn partial_frame_invalidates_all_acknowledgements_and_keeps_damage_for_retry() {
    let (face, plan, main, _, mut compositor) = fixture(1);
    update(&mut compositor, &face, &plan, &main, MAIN);
    let staged = compositor.receipts().next().unwrap().clone();
    let mut display = MemoryDisplay::new();
    display.fail_after = Some(7);
    assert!(compositor.compose_frame(&mut display).is_err());
    assert_eq!(display.writes, 7);
    assert_eq!(compositor.frame_sequence(), 0);
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
    display.fail_after = None;
    compositor.compose_frame(&mut display).unwrap();
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_some());
    // Even validation failure in a later frame must not expose the old ack.
    display.format.width = 0;
    assert!(compositor.compose_frame(&mut display).is_err());
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
}

#[test]
fn focus_overlay_pixels_cannot_stand_in_for_the_surface() {
    let (face, plan, main, _, mut compositor) = fixture(1);
    compositor
        .place_surface(
            MAIN,
            LayoutRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
            0,
        )
        .unwrap();
    update(&mut compositor, &face, &plan, &main, MAIN);
    compositor.focus_surface(MAIN).unwrap();
    let staged = compositor.receipts().next().unwrap().clone();
    let frame = compositor.compose_frame(&mut MemoryDisplay::new()).unwrap();
    assert!(frame.pixels_written > 0);
    assert_eq!(frame.surfaces_composed, 1);
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
}
