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
    let fields: alloc::vec::Vec<_> = top
        .commands()
        .iter()
        .filter(|command| command.payload().contains('\n'))
        .collect();
    #[cfg(not(feature = "native-compositor"))]
    let line_height = 16;
    #[cfg(feature = "native-compositor")]
    let line_height =
        crate::display::typography::metrics(crate::display::typography::TextRole::Body).line_height;
    assert_eq!(fields[0].bounds.height, 2 * line_height);
    assert_eq!(
        fields[1].bounds.y,
        fields[0].bounds.y + fields[0].bounds.height as i16 + 12
    );
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
            .any(|command| command.payload() == "Kind\ntext/upper")
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
            .any(|command| command.payload().starts_with("Documentation\nUppercase"))
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
