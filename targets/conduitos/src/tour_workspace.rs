//! ConduitOS selection of the Tour-owned renderer-neutral workspace.

use conduit_presentation::{
    ApplicationView, GraphicsCommand, GraphicsError, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, LayoutRect, SemanticPresentationRefusal,
};
use conduit_tour_model::{
    TourLayoutRefusal, TourRect, TourWorkspaceLayout, TourWorkspacePhase, TourWorkspaceState,
};

#[path = "tour_workspace_graph.rs"]
mod graph;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourWorkspaceSceneRefusal {
    Presentation(SemanticPresentationRefusal),
    Layout(TourLayoutRefusal),
    MissingRegion,
    Graphics(GraphicsError),
}

pub fn application_view(
    revision: u32,
    phase: TourWorkspacePhase,
) -> Result<ApplicationView, SemanticPresentationRefusal> {
    TourWorkspaceState::canonical(revision, phase)
        .presentation()?
        .lower()
}

pub fn scene(
    width: u16,
    height: u16,
    revision: u32,
    phase: TourWorkspacePhase,
) -> Result<GraphicsScene, TourWorkspaceSceneRefusal> {
    let state = TourWorkspaceState::canonical(revision, phase);
    scene_for_state(width, height, &state)
}

pub fn scene_for_state(
    width: u16,
    height: u16,
    state: &TourWorkspaceState,
) -> Result<GraphicsScene, TourWorkspaceSceneRefusal> {
    let view = state
        .presentation()
        .and_then(|presentation| presentation.lower())
        .map_err(TourWorkspaceSceneRefusal::Presentation)?;
    let layout = TourWorkspaceLayout::default_for(width, height)
        .map_err(TourWorkspaceSceneRefusal::Layout)?;
    let regions = [
        (layout.narrative, "lesson-status"),
        (layout.patchbay, "patchbay"),
        (layout.source, "source"),
        (layout.output, "result"),
    ];
    let mut scene = GraphicsScene::empty();
    for (rect, key) in regions {
        let bounds = graphics_rect(rect)?;
        let node = view
            .nodes
            .iter()
            .find(|node| node.key == key)
            .ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
        let paint = if key == state.focused_key {
            GraphicsPaintRole::Accent
        } else {
            GraphicsPaintRole::Foreground
        };
        scene
            .push(
                GraphicsCommand::rect(bounds, bounds, paint, GraphicsShapeStyle::Stroke)
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        let label = if node.component == conduit_presentation::ApplicationComponent::CodeBlock
            || node.text.is_empty()
        {
            node.value.as_str()
        } else {
            node.text.as_str()
        };
        scene
            .push(
                GraphicsCommand::text(inset(bounds), bounds, paint, label)
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    }
    graph::append(&mut scene, graphics_rect(layout.patchbay)?, state)?;
    Ok(scene)
}

fn graphics_rect(rect: TourRect) -> Result<LayoutRect, TourWorkspaceSceneRefusal> {
    Ok(LayoutRect {
        x: i16::try_from(rect.x).map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?,
        y: i16::try_from(rect.y).map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?,
        width: rect.width,
        height: rect.height,
    })
}

fn inset(rect: LayoutRect) -> LayoutRect {
    LayoutRect {
        x: rect.x.saturating_add(8),
        y: rect.y.saturating_add(8),
        width: rect.width.saturating_sub(16).max(1),
        height: rect.height.saturating_sub(16).max(1),
    }
}

#[cfg(test)]
mod tests {
    use conduit_presentation::{ApplicationComponent, GraphicsCommandKind};
    use conduit_tour_model::CANONICAL_SOURCE;

    use super::*;

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
        assert_eq!(scene.commands().len(), 16);
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
                width: 294,
                height: 480
            }
        );
        assert_eq!(
            frames[1].bounds,
            LayoutRect {
                x: 294,
                y: 0,
                width: 346,
                height: 264
            }
        );
        assert_eq!(frames[1].paint, GraphicsPaintRole::Accent);
        assert_eq!(scene.commands()[5].payload(), CANONICAL_SOURCE);
        assert!(scene.commands()[5].payload().ends_with("}"));
        assert!(scene.commands()[7].payload().contains("Patchbay open"));
        assert!(scene.commands().iter().all(|command| {
            command.clip_class() == conduit_presentation::GraphicsClipClass::FullyVisible
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
        assert_eq!(card.paint, GraphicsPaintRole::Accent);
        assert!(card.payload().contains("value/text"));
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
                .any(|command| command.payload().starts_with("result\npresentation/text"))
        );
    }
}
