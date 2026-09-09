use super::*;

#[test]
fn chooser_button_uses_visible_bounds_and_current_workspace_identity() {
    let (tour, mut shell, mut display) = fixture();
    shell.present(&tour, &mut display).unwrap();
    for (x, y, expected) in [
        (8, 128, true),
        (119, 155, true),
        (7, 128, false),
        (120, 128, false),
        (8, 127, false),
        (8, 156, false),
    ] {
        let route = delivered(shell.route_pointer(x, y, false).unwrap());
        assert_eq!(shell.chooser_open_hit(&route, &tour).unwrap(), expected);
    }
    let route = delivered(shell.route_pointer(12, 140, true).unwrap());
    assert!(shell.chooser_open_hit(&route, &tour).unwrap());
    let shown = shell
        .show_transient(
            &tour,
            TourTransientKind::Chooser,
            "Choose a Patchbay Gear",
            &mut display,
        )
        .unwrap();
    assert_eq!(shown.kind, TourTransientKind::Chooser);
    assert!(shell.has_transient());
    let overlay = delivered(shell.route_pointer(320, 240, false).unwrap());
    assert!(!shell.chooser_open_hit(&overlay, &tour).unwrap());
    shell.present(&tour, &mut display).unwrap();
    assert!(shell.chooser_open_hit(&route, &tour).is_err());
}
