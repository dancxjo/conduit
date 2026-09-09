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

pub(crate) const STATUS_HEIGHT: u16 = 64;

pub(crate) fn chooser_bounds(layout: &TourWorkspaceLayout) -> LayoutRect {
    LayoutRect {
        x: 8,
        y: 128,
        width: layout.narrative.width.saturating_sub(16).clamp(1, 112),
        height: 28,
    }
}

#[cfg(any(test, target_arch = "x86_64"))]
pub(crate) fn hits_card(
    layout: &TourWorkspaceLayout,
    x: u16,
    y: u16,
) -> Result<bool, TourWorkspaceSceneRefusal> {
    let bounds = graphics_rect(layout.patchbay)?;
    Ok((0..3).any(|index| {
        let card = graph::card_bounds(bounds, index);
        let x = i32::from(x);
        let y = i32::from(y);
        x >= i32::from(card.x)
            && x < i32::from(card.x) + i32::from(card.width)
            && y >= i32::from(card.y)
            && y < i32::from(card.y) + i32::from(card.height)
            && x < i32::from(bounds.x) + i32::from(bounds.width)
            && y < i32::from(bounds.y) + i32::from(bounds.height)
    }))
}

pub(crate) fn inspector_width(width: u16) -> u16 {
    (width / 3).max(180)
}

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
    scene_with_observations(width, height, state, None)
}

pub(crate) fn scene_with_observations(
    width: u16,
    height: u16,
    state: &TourWorkspaceState,
    observations: Option<&crate::text_composition::TextObservations>,
) -> Result<GraphicsScene, TourWorkspaceSceneRefusal> {
    let view = state
        .presentation()
        .and_then(|presentation| presentation.lower())
        .map_err(TourWorkspaceSceneRefusal::Presentation)?;
    let layout =
        layout_for_state(width, height, state).map_err(TourWorkspaceSceneRefusal::Layout)?;
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
        let frame = if key == "lesson-status" {
            // Retained pixels outside a reflowed region must not survive a revision.
            let viewport = LayoutRect {
                x: 0,
                y: 0,
                width,
                height,
            };
            GraphicsCommand::rect(
                viewport,
                viewport,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
        } else {
            GraphicsCommand::rect(bounds, bounds, paint, GraphicsShapeStyle::Stroke)
        };
        scene
            .push(frame.map_err(TourWorkspaceSceneRefusal::Graphics)?)
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        let label = if node.component == conduit_presentation::ApplicationComponent::CodeBlock
            || node.text.is_empty()
        {
            node.value.as_str()
        } else {
            node.text.as_str()
        };
        // Keep the portable panel titles visible instead of dropping them
        // while lowering their children into the native pane rectangles.
        let panel_key = match key {
            "lesson-status" => Some("lesson"),
            "source" => Some("source-panel"),
            "result" => Some("result-panel"),
            _ => None,
        };
        let labeled;
        let label = if let Some(panel_key) = panel_key {
            let panel = view
                .nodes
                .iter()
                .find(|node| node.key == panel_key)
                .ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
            labeled = alloc::format!("{}\n\n{label}", panel.text);
            labeled.as_str()
        } else {
            label
        };
        scene
            .push(
                GraphicsCommand::text(inset(bounds), bounds, paint, label)
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    }
    graph::append(
        &mut scene,
        graphics_rect(layout.patchbay)?,
        state,
        observations,
    )?;
    let button = chooser_bounds(&layout);
    let clip = graphics_rect(layout.narrative)?;
    scene
        .push(
            GraphicsCommand::rect(
                button,
                clip,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Stroke,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?,
        )
        .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    scene
        .push(
            GraphicsCommand::text(inset(button), clip, GraphicsPaintRole::Foreground, "Gears")
                .map_err(TourWorkspaceSceneRefusal::Graphics)?,
        )
        .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    Ok(scene)
}

/// Reserve the shell's Inspector/status edges while retaining full-display
/// pointer normalization. Rendering and hit resolution consume this same layout.
pub(crate) fn layout_for_state(
    width: u16,
    height: u16,
    state: &TourWorkspaceState,
) -> Result<TourWorkspaceLayout, TourLayoutRefusal> {
    if state.selected_patchbay_subject.is_none() {
        return TourWorkspaceLayout::default_for(width, height);
    }
    let available_width = width
        .checked_sub(inspector_width(width))
        .ok_or(TourLayoutRefusal::EmptyViewport)?;
    let available_height = height
        .checked_sub(STATUS_HEIGHT)
        .ok_or(TourLayoutRefusal::EmptyViewport)?;
    let mut layout = TourWorkspaceLayout::new(
        available_width,
        available_height,
        30,
        conduit_tour_model::DEFAULT_PATCHBAY_PERCENT,
        conduit_tour_model::DEFAULT_SOURCE_PERCENT,
    )?;
    layout.viewport.width = width;
    layout.viewport.height = height;
    layout.validate()?;
    Ok(layout)
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
        assert_eq!(scene.commands().len(), 21);
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
                height: 264
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
                .starts_with("A first Form\n\n")
        );
        assert!(scene.commands()[7].payload().starts_with("Output\n\n"));
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
                .any(|command| command.payload().starts_with("result\npresentation/text"))
        );
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
}
