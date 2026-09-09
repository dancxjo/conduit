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
    let bottom =
        inspector_scene(bounds, &presentation, SCROLL_CONTENT_HEIGHT - bounds.height).unwrap();
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
