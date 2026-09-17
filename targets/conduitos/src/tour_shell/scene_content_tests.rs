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
    let close = top
        .commands()
        .iter()
        .find(|command| command.payload() == "Close")
        .unwrap();
    assert_eq!(
        close.bounds.x,
        super::super::controls::inspector_close_bounds(bounds.width).x
            + (crate::display::SPACE_SM * 2 + crate::display::ICON_SM) as i16
    );
    assert!(top.commands().iter().any(|command| {
        command.style == GraphicsShapeStyle::RoundedStroke
            && command.bounds == super::super::controls::inspector_close_bounds(bounds.width)
    }));
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
fn transient_chooser_uses_the_shared_icon_button_language() {
    let state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
    let presentation = state
        .transient_presentation(
            conduit_tour_model::TourTransientKind::Chooser,
            "Choose a Patchbay Gear",
        )
        .unwrap();
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 420,
        height: 320,
    };
    let scene = transient_scene(bounds, &presentation, 0).unwrap();
    let rounded = scene
        .commands()
        .iter()
        .filter(|command| command.style == GraphicsShapeStyle::RoundedStroke)
        .count();
    assert_eq!(
        rounded,
        conduit_tour_model::CANONICAL_PATCHBAY_GEARS.len() + 1
    );
    assert_eq!(
        scene
            .commands()
            .iter()
            .filter(|command| command.kind == conduit_presentation::GraphicsCommandKind::Icon)
            .count(),
        conduit_tour_model::CANONICAL_PATCHBAY_GEARS.len() + 1
    );
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Close")
    );
}

#[test]
fn non_chooser_transients_render_meaningful_content_without_placeholder_rows() {
    let state = TourWorkspaceState::canonical(1, TourWorkspacePhase::LessonReady);
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 640,
        height: 256,
    };
    for (kind, title, detail) in [
        (
            conduit_tour_model::TourTransientKind::Confirmation,
            "Confirmation",
            "Play completed",
        ),
        (
            conduit_tour_model::TourTransientKind::Refusal,
            "Refusal detail",
            "Action refused",
        ),
    ] {
        let presentation = state.transient_presentation(kind, detail).unwrap();
        let scene = transient_scene(bounds, &presentation, 0).unwrap();
        for label in [title, detail, "Close"] {
            assert!(
                scene
                    .commands()
                    .iter()
                    .any(|command| command.payload() == label),
                "missing visible {label}"
            );
        }
        assert!(scene.commands().iter().all(|command| {
            command.paint != conduit_presentation::GraphicsPaintRole::Status
                || command.style != GraphicsShapeStyle::Fill
        }));
        assert!(scene.commands().iter().all(|command| {
            command.paint != conduit_presentation::GraphicsPaintRole::Background
                || command.style != GraphicsShapeStyle::Fill
                || command.bounds == bounds
        }));
        assert!(transient_scene(bounds, &presentation, 1).is_err());
    }
    let mut missing_close = state
        .transient_presentation(
            conduit_tour_model::TourTransientKind::Confirmation,
            "Play completed",
        )
        .unwrap();
    missing_close
        .subjects
        .retain(|subject| subject.identity != conduit_tour_model::TRANSIENT_CLOSE_ACTION_ID);
    assert!(transient_scene(bounds, &missing_close, 0).is_err());
    let mut blank_detail = state
        .transient_presentation(
            conduit_tour_model::TourTransientKind::Refusal,
            "Action refused",
        )
        .unwrap();
    blank_detail.text[0].text.clear();
    assert!(transient_scene(bounds, &blank_detail, 0).is_err());
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
