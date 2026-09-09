//! Hosted lifecycle workload for preparation-memory integration. This does
//! not itself prove native allocator reuse or long-running emulator behavior.
use super::*;

#[test]
fn repeated_revisions_preserve_live_surfaces_and_refuse_retired_routes() {
    let (mut tour, mut shell, mut display) = fixture();
    let mut frame = shell
        .present(&tour, &mut display)
        .unwrap()
        .frame
        .frame_sequence;
    let retired = delivered(shell.route_pointer(10, 10, false).unwrap());
    for iteration in 0..64 {
        let gear = conduit_tour_model::CANONICAL_PATCHBAY_GEARS[iteration % 3];
        tour.select_gear(tour.controller().state().revision, gear)
            .unwrap();
        let selected = shell.present(&tour, &mut display).unwrap();
        assert!(selected.inspector.is_some());
        assert!(selected.frame.frame_sequence > frame);
        assert_eq!(shell.compositor.receipts().count(), 3);
        assert!(shell.validate_pointer_route(&retired).is_err());

        shell
            .show_transient(
                &tour,
                TourTransientKind::Chooser,
                "Choose a Patchbay Gear",
                &mut display,
            )
            .unwrap();
        assert_eq!(shell.compositor.receipts().count(), 4);
        let overlay = delivered(shell.route_pointer(320, 240, true).unwrap());
        assert_eq!(overlay.surface_id, TRANSIENT_SURFACE);
        shell.dismiss_transient(&mut display).unwrap();
        assert!(shell.validate_pointer_route(&overlay).is_err());
        assert_eq!(shell.compositor.receipts().count(), 3);

        assert!(tour.dismiss_inspector().unwrap());
        let dismissed = shell.present(&tour, &mut display).unwrap();
        assert!(dismissed.inspector.is_none());
        assert_eq!(shell.compositor.receipts().count(), 2);
        assert!(dismissed.frame.frame_sequence > selected.frame.frame_sequence);
        frame = dismissed.frame.frame_sequence;
    }
    shell.suspend().unwrap();
    assert_eq!(shell.compositor.receipts().count(), 0);
    assert_eq!(
        shell.route_pointer(10, 10, false).unwrap(),
        InputRoute::NoTarget
    );
}
