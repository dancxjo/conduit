use conduit_face::{ApplicationComponent, GraphicsCommandKind};
use conduit_tour_model::CANONICAL_SOURCE;

use super::*;

#[test]
fn chooser_label_fits_a_complete_measured_font_line() {
    for (width, height) in [(640, 480), (1280, 800)] {
        let scene = scene(width, height, 1, TourWorkspacePhase::PatchbayOpen).unwrap();
        let label = scene
            .commands()
            .iter()
            .find(|command| command.payload() == "Gears")
            .unwrap();
        let measured =
            crate::display::text_height(label.payload(), label.bounds.width).unwrap();
        assert!(label.bounds.height >= measured);
        assert!(label.bounds.y >= label.clip.y);
        assert!(
            i32::from(label.bounds.y) + i32::from(measured)
                <= i32::from(label.clip.y) + i32::from(label.clip.height)
        );
    }
}

#[test]
fn conduitos_consumes_the_tour_owned_portable_view() {
    let view = application_view(11, TourWorkspacePhase::PatchbayOpen).unwrap();
    assert_eq!(view.revision, 11);
    assert!(view.nodes.iter().any(|node| {
        node.key == "patchbay" && node.component == ApplicationComponent::PatchbayCanvas
    }));
    assert!(view.actions.iter().any(|action| action.id == "tour.run"));
}

#[test]
fn native_scene_manifests_every_shared_region_and_visible_focus() {
    let scene = scene(640, 480, 12, TourWorkspacePhase::PatchbayOpen).unwrap();
    assert!(scene.commands().len() > 16);
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload().contains("Conduit lets you make"))
    );
    let frames: alloc::vec::Vec<_> = scene
        .commands()
        .iter()
        .take(8)
        .filter(|command| command.kind == GraphicsCommandKind::Rect)
        .collect();
    assert_eq!(frames.len(), 4);
    assert_eq!(
        frames[0].bounds,
        LayoutRect {
            x: 0,
            y: 0,
            width: 640,
            height: 480
        }
    );
    assert_eq!(
        frames[1].bounds,
        LayoutRect {
            x: 294,
            y: 0,
            width: 346,
            #[cfg(feature = "native-compositor")]
            height: 211,
            #[cfg(not(feature = "native-compositor"))]
            height: 228,
        }
    );
    assert_eq!(frames[1].paint, GraphicsPaintRole::Accent);
    assert_eq!(
        scene.commands()[5].payload(),
        alloc::format!("Source\n\n{CANONICAL_SOURCE}")
    );
    assert!(
        scene.commands()[1]
            .payload()
            .starts_with("One Program, Many Computers\n\n")
    );
    assert!(scene.commands()[7].payload().starts_with("Output\n\n"));
    assert!(scene.commands()[5].payload().ends_with("}."));
    assert!(scene.commands()[7].payload().contains("Patchbay open"));
    assert!(scene.commands().iter().take(16).all(|command| {
        command.clip_class() == conduit_face::GraphicsClipClass::FullyVisible
    }));
}

#[test]
fn native_focus_tracks_the_shared_phase_key() {
    for (phase, focused_frame) in [
        (TourWorkspacePhase::LessonReady, 2),
        (TourWorkspacePhase::ResultVisible, 3),
        (TourWorkspacePhase::PatchbayOpen, 1),
    ] {
        let scene = scene(640, 480, 1, phase).unwrap();
        let frames: alloc::vec::Vec<_> = scene
            .commands()
            .iter()
            .take(8)
            .filter(|command| command.kind == GraphicsCommandKind::Rect)
            .collect();
        assert_eq!(
            frames
                .iter()
                .position(|frame| frame.paint == GraphicsPaintRole::Accent),
            Some(focused_frame)
        );
    }
}

#[test]
fn native_scene_refuses_an_empty_viewport() {
    assert_eq!(
        scene(0, 480, 1, TourWorkspacePhase::LessonReady),
        Err(TourWorkspaceSceneRefusal::Layout(
            TourLayoutRefusal::EmptyViewport
        ))
    );
}

#[test]
fn graph_cards_project_catalog_ports_and_selected_gear() {
    let mut state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
    state.selected_patchbay_subject = Some("meet-one-gear/change".into());
    let scene = scene_for_state(1280, 800, &state).unwrap();
    let card = scene
        .commands()
        .iter()
        .find(|command| command.payload().starts_with("change\ntext/upper"))
        .unwrap();
    assert_eq!(card.paint, GraphicsPaintRole::Selected);
    assert!(card.payload().contains("Selected\n"));
    assert!(scene.commands().iter().any(|command| {
        command.payload().contains("value/text")
            && command.paint == GraphicsPaintRole::Foreground
    }));
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload().starts_with("words\ntext/literal"))
    );
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload().starts_with("result\nface/text"))
    );
}

#[test]
fn default_workspace_reserves_status_without_rescaling_pointer_space() {
    let state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
    let layout = layout_for_state(640, 480, &state).unwrap();
    assert_eq!(layout.viewport.width, 640);
    assert_eq!(layout.viewport.height, 480);
    for rect in [
        layout.narrative,
        layout.patchbay,
        layout.source,
        layout.output,
    ] {
        assert!(rect.x + rect.width <= 640);
        assert!(rect.y + rect.height <= 480 - STATUS_HEIGHT);
    }
}

#[test]
fn selected_workspace_reserves_inspector_and_status_without_rescaling_pointer_space() {
    let mut state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
    state.selected_patchbay_subject = Some("meet-one-gear/change".into());
    let layout = layout_for_state(1280, 800, &state).unwrap();
    assert_eq!(layout.viewport.width, 1280);
    assert_eq!(layout.viewport.height, 800);
    for rect in [
        layout.narrative,
        layout.patchbay,
        layout.source,
        layout.output,
    ] {
        assert!(rect.x + rect.width <= 1280 - inspector_width(1280));
        assert!(rect.y + rect.height <= 800 - STATUS_HEIGHT);
    }
}
