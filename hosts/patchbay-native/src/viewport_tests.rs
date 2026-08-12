use super::{Arguments, PatchbayApplication, BACKGROUND};
use embedded_graphics::geometry::Point;
use winit::keyboard::{Key, ModifiersState, NamedKey};

fn empty_form_application(label: &str) -> (PatchbayApplication, std::path::PathBuf) {
    let directory =
        std::env::temp_dir().join(format!("patchbay-viewport-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("making.conduit");
    std::fs::write(&path, "form making {\n}\n").unwrap();
    let application = PatchbayApplication::new(Arguments {
        form_path: Some(path),
        ..Arguments::default()
    })
    .unwrap();
    (application, directory)
}

fn initialize_viewport(application: &mut PatchbayApplication) {
    application
        .canvas_viewport
        .resize(super::gui::canvas_rect(1_100, 720))
        .unwrap();
}

fn redraw(application: &mut PatchbayApplication) {
    let mut pixels = vec![BACKGROUND; 1_100 * 720];
    application.hit_targets = super::gui::draw_patchbay(
        &mut pixels,
        1_100,
        720,
        application.graphical_form.as_ref().unwrap(),
        super::gui::PatchbayViewContext {
            selected: application.selected_graphical_identity(),
            breadcrumb: "",
            lifecycle: &Default::default(),
            palette: &application.palette,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &application.layout,
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &application.canvas_viewport,
        },
    );
}

#[test]
fn transformed_pointer_placement_and_drag_round_trip_to_canonical_layout() {
    let (mut application, directory) = empty_form_application("authoring");
    initialize_viewport(&mut application);
    application
        .canvas_viewport
        .pan(Point::new(96, -48))
        .unwrap();
    application.cursor_position = (550.0, 320.0);
    application
        .canvas_viewport
        .zoom_by(250, Point::new(550, 320))
        .unwrap();

    let placement_cursor = application
        .canvas_viewport
        .world_to_screen(Point::new(495, 320))
        .unwrap();
    application.palette_drag = Some("text/upper".into());
    application.cursor_position = (f64::from(placement_cursor.x), f64::from(placement_cursor.y));
    application.handle_canvas_release().unwrap();
    let gear_identity = application.graphical_form.as_ref().unwrap().gears[0]
        .identity
        .clone();
    assert_eq!(
        application.layout.position(&gear_identity),
        Some((400, 300))
    );

    redraw(&mut application);
    let gear_target = application
        .hit_targets
        .iter()
        .find(|target| {
            matches!(&target.action, super::gui::GuiAction::SelectSubject(subject) if subject.subject_identity == gear_identity)
        })
        .unwrap();
    let press = application
        .canvas_viewport
        .world_to_screen(Point::new(430, 315))
        .unwrap();
    assert!(gear_target.contains(f64::from(press.x), f64::from(press.y)));
    application.cursor_position = (f64::from(press.x), f64::from(press.y));
    application.handle_canvas_press().unwrap();
    let release = application
        .canvas_viewport
        .world_to_screen(Point::new(695, 420))
        .unwrap();
    application.cursor_position = (f64::from(release.x), f64::from(release.y));
    application.handle_canvas_release().unwrap();
    assert_eq!(
        application.layout.position(&gear_identity),
        Some((600, 400))
    );

    std::fs::remove_file(directory.join("making.conduit")).unwrap();
    let sidecar = directory.join("making.conduit.patchbay.json");
    if sidecar.exists() {
        std::fs::remove_file(sidecar).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn keyboard_pan_zoom_fit_center_and_reset_change_only_viewport_state() {
    let (mut application, directory) = empty_form_application("keyboard");
    initialize_viewport(&mut application);
    application.palette.focus();
    application.palette.append("uppercase").unwrap();
    assert!(application.handle_palette_key(&Key::Named(NamedKey::Enter)));
    let graph_before = application.graphical_form.clone().unwrap();
    let layout_before = application.layout.clone();
    let palette_before = application.palette.clone();

    application.modifiers = ModifiersState::CONTROL;
    application.cursor_position = (500.0, 300.0);
    assert!(application.handle_viewport_key(&Key::Character("+".into())));
    assert_eq!(application.canvas_viewport.zoom_per_mille(), 1_125);
    assert!(application.handle_viewport_key(&Key::Character("f".into())));
    assert!(application.handle_viewport_key(&Key::Character("c".into())));
    application.modifiers = ModifiersState::SHIFT;
    assert!(application.handle_viewport_key(&Key::Named(NamedKey::ArrowRight)));
    assert_ne!(application.canvas_viewport.offset(), Point::zero());
    application.modifiers = ModifiersState::CONTROL;
    assert!(application.handle_viewport_key(&Key::Character("0".into())));
    assert_eq!(application.canvas_viewport.zoom_per_mille(), 1_000);
    assert_eq!(application.canvas_viewport.offset(), Point::zero());
    assert_eq!(application.graphical_form.as_ref().unwrap(), &graph_before);
    assert_eq!(application.layout, layout_before);
    assert_eq!(application.palette, palette_before);

    std::fs::remove_file(directory.join("making.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn resize_preserves_canvas_world_center_without_touching_layout() {
    let (mut application, directory) = empty_form_application("resize");
    initialize_viewport(&mut application);
    application
        .canvas_viewport
        .pan(Point::new(120, -80))
        .unwrap();
    let old_center = Point::new(496, 365);
    let world_center = application
        .canvas_viewport
        .screen_to_world(old_center)
        .unwrap();
    let layout_before = application.layout.clone();
    application
        .canvas_viewport
        .resize(super::gui::canvas_rect(1_300, 820))
        .unwrap();
    let new_center = Point::new(596, 415);
    assert_eq!(
        application
            .canvas_viewport
            .screen_to_world(new_center)
            .unwrap(),
        world_center
    );
    assert_eq!(application.layout, layout_before);

    std::fs::remove_file(directory.join("making.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn visible_pointer_controls_expose_zoom_fit_center_and_reset() {
    use super::gui_hit::ViewportAction;

    let (mut application, directory) = empty_form_application("pointer-controls");
    initialize_viewport(&mut application);
    application.palette.focus();
    application.palette.append("uppercase").unwrap();
    assert!(application.handle_palette_key(&Key::Named(NamedKey::Enter)));
    redraw(&mut application);

    for action in [
        ViewportAction::ZoomOut,
        ViewportAction::ZoomIn,
        ViewportAction::Fit,
        ViewportAction::CenterSelection,
        ViewportAction::Reset,
    ] {
        let point = (26..46)
            .flat_map(|y| (300..540).map(move |x| (x, y)))
            .find(|(x, y)| {
                application.hit_targets.iter().rev().any(|target| {
                    target.contains(f64::from(*x), f64::from(*y))
                        && matches!(target.action, super::gui::GuiAction::Viewport(candidate) if candidate == action)
                })
            })
            .expect("visible viewport pointer control");
        application.cursor_position = (f64::from(point.0), f64::from(point.1));
        application.handle_canvas_press().unwrap();
    }
    assert_eq!(application.canvas_viewport.zoom_per_mille(), 1_000);
    assert_eq!(application.canvas_viewport.offset(), Point::zero());

    std::fs::remove_file(directory.join("making.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
