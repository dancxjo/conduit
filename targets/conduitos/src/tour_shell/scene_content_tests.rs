use super::*;
use conduit_tour_model::{TourWorkspacePhase, TourWorkspaceState};

#[test]
fn status_cells_preserve_missing_evidence_as_distinct_values() {
    let state = TourWorkspaceState::canonical(1, TourWorkspacePhase::LessonReady);
    let presentation = state.status_presentation().unwrap();
    let scene = status_scene(
        LayoutRect {
            x: 0,
            y: 0,
            width: 1280,
            height: 64,
        },
        &presentation,
    )
    .unwrap();
    assert_eq!(scene.commands().len(), 13);
    let frames: alloc::vec::Vec<_> = scene
        .commands()
        .iter()
        .filter(|command| command.style == GraphicsShapeStyle::Stroke)
        .collect();
    assert_eq!(frames.len(), 6);
    for pair in frames.windows(2) {
        assert!(
            i32::from(pair[0].bounds.x) + i32::from(pair[0].bounds.width)
                < i32::from(pair[1].bounds.x)
        );
    }
    assert!(frames.iter().all(|frame| frame.bounds.height == 60));
    for expected in [
        "Body\nAbsent",
        "Wake\nAbsent",
        "Plan\nAbsent",
        "Play\nInactive",
        "Lines\nUnobserved",
        "Host\nUnobserved",
    ] {
        assert!(
            scene
                .commands()
                .iter()
                .any(|command| command.payload() == expected)
        );
    }
}

#[test]
fn inspection_renders_catalog_fields_and_scrolls_to_documentation() {
    let mut state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
    state.selected_patchbay_subject = Some("meet-one-gear/change".into());
    let presentation = state.inspector_presentation().unwrap().unwrap();
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 420,
        height: 320,
    };
    let top = inspector_scene(bounds, &presentation, 0).unwrap();
    let kind = top
        .commands()
        .iter()
        .find(|command| command.payload() == "Kind")
        .unwrap();
    let value = top
        .commands()
        .iter()
        .find(|command| command.payload() == "text/upper")
        .unwrap();
    let face = top
        .commands()
        .iter()
        .find(|command| command.payload() == "Face")
        .unwrap();
    assert_eq!(kind.bounds.height, 16);
    assert_eq!(kind.paint, GraphicsPaintRole::Accent);
    assert_eq!(value.paint, GraphicsPaintRole::Foreground);
    assert_eq!(value.bounds.y, kind.bounds.y + 16);
    assert_eq!(face.bounds.y, kind.bounds.y + 44);
    let narrow = LayoutRect {
        width: 180,
        ..bounds
    };
    let wide_extent = super::super::fields::project(bounds, &presentation, 0, None).unwrap();
    let narrow_extent = super::super::fields::project(narrow, &presentation, 0, None).unwrap();
    assert!(narrow_extent > wide_extent);
    assert!(
        top.commands()
            .iter()
            .any(|command| command.payload() == "text/upper")
    );
    assert!(
        !top.commands()
            .iter()
            .any(|command| command.payload().starts_with("Documentation"))
    );
    let extent = super::super::fields::project(bounds, &presentation, 0, None).unwrap();
    let bottom =
        inspector_scene(bounds, &presentation, extent.saturating_sub(bounds.height)).unwrap();
    assert!(
        bottom
            .commands()
            .iter()
            .any(|command| command.payload() == "Documentation")
    );
    assert!(
        bottom
            .commands()
            .iter()
            .any(|command| command.payload().starts_with("Uppercase"))
    );
    assert!(
        bottom
            .commands()
            .iter()
            .all(|command| !command.payload().contains("HELLO"))
    );
}

#[test]
fn inspection_rejects_oversized_fields_even_when_scrolled_out_of_view() {
    let mut state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
    state.selected_patchbay_subject = Some("meet-one-gear/change".into());
    let mut presentation = state.inspector_presentation().unwrap().unwrap();
    presentation.text[0].text = "x".repeat(193);
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 180,
        height: 240,
    };
    assert!(inspector_scene(bounds, &presentation, 500).is_err());
    assert!(super::super::fields::project(bounds, &presentation, 0, None).is_err());
}
