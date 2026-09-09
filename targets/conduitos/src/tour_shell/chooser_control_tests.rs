use super::*;

#[test]
fn completed_run_is_not_an_active_native_control() {
    let (mut tour, mut shell, mut display) = fixture();
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = host_offer(&identities);
    tour.accept(
        &ApplicationEvent {
            revision: tour.controller().state().revision,
            action: conduit_tour_model::RUN_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: vec![],
        },
        &identities,
        &offer,
        "build",
        &mut Clock::default(),
        &mut Serial::default(),
        &mut Interrupts::default(),
        &mut Idle::default(),
    )
    .unwrap();
    assert!(!tour.controller().state().run_action_available());
    shell.present(&tour, &mut display).unwrap();
    let route = delivered(shell.route_pointer(8, 92, false).unwrap());
    assert!(!shell.run_hit(&route, &tour).unwrap());
    let scene = tour.scene(640, 480).unwrap();
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Run unavailable")
    );
    assert!(
        !scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Run Plan")
    );
}

#[test]
fn run_button_has_exact_bounds_and_refuses_a_retired_route() {
    let (tour, mut shell, mut display) = fixture();
    shell.present(&tour, &mut display).unwrap();
    for (x, y, expected) in [
        (8, 92, true),
        (119, 119, true),
        (7, 92, false),
        (120, 92, false),
        (8, 91, false),
        (8, 120, false),
    ] {
        let crate::native_compositor::InputRoute::Delivered(route) =
            shell.route_pointer(x, y, false).unwrap()
        else {
            panic!("workspace route");
        };
        assert_eq!(shell.run_hit(&route, &tour).unwrap(), expected);
    }
    let crate::native_compositor::InputRoute::Delivered(route) =
        shell.route_pointer(8, 92, false).unwrap()
    else {
        panic!("workspace route");
    };
    shell.suspend().unwrap();
    assert!(shell.run_hit(&route, &tour).is_err());
    let scene = tour.scene(640, 480).unwrap();
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Run Plan")
    );
    assert_eq!(scene.commands().len(), 23);
}

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
fn chooser_has_a_visible_bounded_outer_frame() {
    let (tour, _, _) = fixture();
    let presentation = tour
        .controller()
        .state()
        .transient_presentation(TourTransientKind::Chooser, "Choose a Patchbay Gear")
        .unwrap();
    let bounds = conduit_presentation::LayoutRect {
        x: 160,
        y: 160,
        width: 320,
        height: 160,
    };
    let scene = transient_scene(bounds, &presentation, 0).unwrap();
    let frame = &scene.commands()[1];
    assert_eq!(frame.kind, conduit_presentation::GraphicsCommandKind::Rect);
    assert_eq!(
        frame.style,
        conduit_presentation::GraphicsShapeStyle::Stroke
    );
    assert_eq!(frame.paint, conduit_presentation::GraphicsPaintRole::Accent);
    assert_eq!(
        frame.bounds,
        conduit_presentation::LayoutRect {
            x: 0,
            y: 0,
            width: 320,
            height: 160
        }
    );
    assert_eq!(frame.clip, frame.bounds);
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Close")
    );
    assert!(presentation.subjects.iter().any(|subject| subject.identity
        == conduit_tour_model::TRANSIENT_CLOSE_ACTION_ID
        && subject.role == conduit_presentation::PresentationRole::Action));
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
