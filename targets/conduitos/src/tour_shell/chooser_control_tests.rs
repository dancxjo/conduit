use super::*;

#[test]
fn compact_rows_are_visible_and_each_resolves_its_own_gear() {
    for height in [160, 266, 360] {
        let bounds = conduit_presentation::LayoutRect {
            x: 0,
            y: 0,
            width: 320,
            height,
        };
        assert!(super::super::chooser_layout::content_height(bounds) <= height);
        for index in 0..3 {
            let row = super::super::chooser_layout::row(bounds, index, 0);
            assert_eq!(
                super::super::chooser_layout::hit(bounds, 0, row.x as u16, row.y as u16),
                Some(index as u8)
            );
            assert_eq!(
                super::super::chooser_layout::hit(
                    bounds,
                    0,
                    row.x as u16,
                    row.y as u16 + row.height
                ),
                None
            );
        }
    }
}

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
