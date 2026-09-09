use super::*;

#[test]
fn invalid_replacement_content_preserves_the_current_dialog_and_route() {
    let (tour, mut shell, mut display) = fixture();
    shell.present(&tour, &mut display).unwrap();
    shell
        .show_transient(
            &tour,
            TourTransientKind::Chooser,
            "Choose a Patchbay Gear",
            &mut display,
        )
        .unwrap();
    let route = delivered(shell.route_pointer(200, 240, true).unwrap());
    let gear = shell.chooser_gear(&route).unwrap();
    assert!(gear.is_some());
    let frame = shell.compositor.frame_sequence();
    assert!(
        shell
            .show_transient(
                &tour,
                TourTransientKind::Chooser,
                &"x".repeat(193),
                &mut display
            )
            .is_err()
    );
    assert!(shell.has_transient());
    assert_eq!(shell.compositor.frame_sequence(), frame);
    assert_eq!(shell.chooser_gear(&route).unwrap(), gear);
}
