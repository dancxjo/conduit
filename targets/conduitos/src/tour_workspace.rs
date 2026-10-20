//! ConduitOS selection of the Tour-owned renderer-neutral workspace.

use conduit_face::{
    ApplicationView, GraphicsCommand, GraphicsError, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, LayoutRect, SemanticFaceRefusal,
};
use conduit_tour_model::{
    TourLayoutRefusal, TourRect, TourWorkspaceLayout, TourWorkspacePhase, TourWorkspaceState,
};

#[path = "tour_workspace_graph.rs"]
mod graph;
#[path = "tour_workspace_lesson.rs"]
pub(crate) mod lesson;

// Six compact cells retain their icon gutters. At 640px the Host's three
// logical lines occupy four measured Label lines; reserve the same metrics
// used by the status painter rather than assuming the rescue line height.
#[cfg(feature = "native-compositor")]
pub(crate) const STATUS_HEIGHT: u16 =
    4 * crate::display::typography::metrics(crate::display::typography::TextRole::Label)
        .line_height
        + 2 * crate::display::SPACE_SM;
#[cfg(not(feature = "native-compositor"))]
pub(crate) const STATUS_HEIGHT: u16 = 64;
pub(crate) const LESSON_CONTROL_BAR_HEIGHT: u16 = 40;

fn lesson_button_width(layout: &TourWorkspaceLayout) -> u16 {
    (layout.narrative.width.saturating_sub(24) / 2).clamp(1, 112)
}

pub(crate) fn chooser_bounds(layout: &TourWorkspaceLayout) -> LayoutRect {
    let width = lesson_button_width(layout);
    LayoutRect {
        x: i16::try_from(16_u16.saturating_add(width)).unwrap_or(i16::MAX),
        y: i16::try_from(
            layout
                .narrative
                .height
                .saturating_sub(LESSON_CONTROL_BAR_HEIGHT)
                .saturating_add(6),
        )
        .unwrap_or(i16::MAX),
        width,
        height: 28,
    }
}

pub(crate) fn run_bounds(layout: &TourWorkspaceLayout) -> LayoutRect {
    LayoutRect {
        x: 8,
        width: lesson_button_width(layout),
        ..chooser_bounds(layout)
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
    Face(SemanticFaceRefusal),
    Layout(TourLayoutRefusal),
    MissingRegion,
    Graph,
    Graphics(GraphicsError),
}

pub fn application_view(
    revision: u32,
    phase: TourWorkspacePhase,
) -> Result<ApplicationView, SemanticFaceRefusal> {
    TourWorkspaceState::canonical(revision, phase)
        .face()?
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
    let graph = canonical_graph()?;
    scene_with_graph(width, height, state, &graph, observations)
}

/// Preparation only. The live Tour retains this projection across revisions.
pub(crate) fn canonical_graph() -> Result<patchbay_graph::PatchbayGraph, TourWorkspaceSceneRefusal>
{
    let plot = crate::ordinary_plot::checked_expanded_text_plot_named(
        conduit_tour_model::CANONICAL_SOURCE,
        "meet-one-gear",
    )
    .map_err(|_| TourWorkspaceSceneRefusal::Graph)?;
    patchbay_graph::PatchbayGraph::from_expanded(&plot)
        .map_err(|_| TourWorkspaceSceneRefusal::Graph)
}

pub(crate) fn scene_with_graph(
    width: u16,
    height: u16,
    state: &TourWorkspaceState,
    graph: &patchbay_graph::PatchbayGraph,
    observations: Option<&crate::text_composition::TextObservations>,
) -> Result<GraphicsScene, TourWorkspaceSceneRefusal> {
    let view = state
        .face()
        .and_then(|face| face.lower())
        .map_err(TourWorkspaceSceneRefusal::Face)?;
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
        let label = if node.component == conduit_face::ApplicationComponent::CodeBlock
            || node.text.is_empty()
        {
            node.value.as_str()
        } else {
            node.text.as_str()
        };
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
        append_panel_text(&mut scene, bounds, paint, label)?;
    }
    graph::append(
        &mut scene,
        graphics_rect(layout.patchbay)?,
        state,
        graph,
        observations,
    )?;
    let narrative = graphics_rect(layout.narrative)?;
    lesson::append(
        &mut scene,
        LayoutRect {
            height: narrative
                .height
                .min(height.saturating_sub(STATUS_HEIGHT).max(1)),
            ..narrative
        },
        state,
    )?;
    let run_label = match state.run_availability() {
        conduit_face::ActionAvailability::Available => "Run Plan",
        conduit_face::ActionAvailability::Busy { .. }
        | conduit_face::ActionAvailability::Unavailable { .. } => "Run inactive",
    };
    let run = run_bounds(&layout);
    scene
        .push(
            GraphicsCommand::text(run, narrative, GraphicsPaintRole::Accent, run_label)
                .map_err(TourWorkspaceSceneRefusal::Graphics)?,
        )
        .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    let button = chooser_bounds(&layout);
    scene
        .push(
            GraphicsCommand::text(
                LayoutRect {
                    x: button.x.saturating_add(8),
                    y: button.y.saturating_add(4),
                    width: button.width.saturating_sub(16).max(1),
                    height: button.height.saturating_sub(8).max(1),
                },
                narrative,
                GraphicsPaintRole::Foreground,
                "Gears",
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?,
        )
        .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    Ok(scene)
}

fn append_panel_text(
    scene: &mut GraphicsScene,
    clip: LayoutRect,
    paint: GraphicsPaintRole,
    text: &str,
) -> Result<(), TourWorkspaceSceneRefusal> {
    let mut bounds = inset(clip);
    let mut remaining = text;
    while !remaining.is_empty() {
        let end = lesson::chunk_end(remaining);
        let chunk = &remaining[..end];
        let height = lesson::measured_text_height(chunk, bounds.width)?;
        scene
            .push(
                GraphicsCommand::text(
                    LayoutRect {
                        height: height.min(bounds.height).max(1),
                        ..bounds
                    },
                    clip,
                    paint,
                    chunk,
                )
                .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        bounds.y = bounds
            .y
            .checked_add(
                i16::try_from(height).map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?,
            )
            .ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
        bounds.height = bounds.height.saturating_sub(height).max(1);
        remaining = remaining[end..].trim_start();
    }
    Ok(())
}

/// Reserve the shell's status edge in every workspace state and reserve the
/// Inspector edge only while an Inspector is actually present. Keep the
/// viewport at full-display coordinates so rendering and pointer normalization
/// continue to share one coordinate space.
pub(crate) fn layout_for_state(
    width: u16,
    height: u16,
    state: &TourWorkspaceState,
) -> Result<TourWorkspaceLayout, TourLayoutRefusal> {
    let available_height = height
        .checked_sub(STATUS_HEIGHT)
        .ok_or(TourLayoutRefusal::EmptyViewport)?;
    let (available_width, narrative_percent) = if state.selected_patchbay_subject.is_some() {
        (
            width
                .checked_sub(inspector_width(width))
                .ok_or(TourLayoutRefusal::EmptyViewport)?,
            30,
        )
    } else {
        (width, conduit_tour_model::DEFAULT_NARRATIVE_PERCENT)
    };
    let mut layout = TourWorkspaceLayout::new(
        available_width,
        available_height,
        narrative_percent,
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
#[path = "tour_workspace/tests.rs"]
mod tests;
