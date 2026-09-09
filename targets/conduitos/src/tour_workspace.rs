//! ConduitOS selection of the Tour-owned renderer-neutral workspace.

use conduit_presentation::{
    ApplicationView, GraphicsCommand, GraphicsError, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, LayoutRect, SemanticPresentationRefusal,
};
use conduit_tour_model::{
    CANONICAL_PATCHBAY_GEARS, TourLayoutRefusal, TourRect, TourWorkspaceLayout, TourWorkspacePhase,
    TourWorkspaceState,
};

pub const MAX_TOUR_COMPOSITION_LAYERS: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TourWorkspaceComposition {
    layers: [GraphicsScene; MAX_TOUR_COMPOSITION_LAYERS],
}

impl TourWorkspaceComposition {
    pub fn layers(&self) -> &[GraphicsScene] {
        &self.layers
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourWorkspaceSceneRefusal {
    Presentation(SemanticPresentationRefusal),
    Layout(TourLayoutRefusal),
    MissingRegion,
    Graphics(GraphicsError),
}

impl TourWorkspaceSceneRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Presentation(_) => "tour-presentation-refused",
            Self::Layout(_) => "tour-layout-refused",
            Self::MissingRegion => "tour-region-missing",
            Self::Graphics(_) => "tour-graphics-refused",
        }
    }
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
        let label = label
            .get(
                ..label
                    .len()
                    .min(conduit_presentation::MAX_GRAPHICS_TEXT_BYTES),
            )
            .ok_or(TourWorkspaceSceneRefusal::MissingRegion)?;
        scene
            .push(
                GraphicsCommand::text(inset(bounds), bounds, paint, label)
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    }
    Ok(scene)
}

pub fn composition_for_state(
    width: u16,
    height: u16,
    state: &TourWorkspaceState,
) -> Result<TourWorkspaceComposition, TourWorkspaceSceneRefusal> {
    let base = scene_for_state(width, height, state)?;
    let layout = TourWorkspaceLayout::default_for(width, height)
        .map_err(TourWorkspaceSceneRefusal::Layout)?;
    let patchbay = patchbay_scene(layout.patchbay, state)?;
    Ok(TourWorkspaceComposition {
        layers: [base, patchbay],
    })
}

fn patchbay_scene(
    region: TourRect,
    state: &TourWorkspaceState,
) -> Result<GraphicsScene, TourWorkspaceSceneRefusal> {
    let clip = graphics_rect(region)?;
    let inset_x = 14_u16;
    let usable_width = region.width.saturating_sub(inset_x.saturating_mul(2));
    let gear_width = (usable_width.saturating_sub(32) / 3).max(1);
    let gear_height = region.height.saturating_sub(64).clamp(24, 72);
    let gear_y = region
        .y
        .saturating_add(region.height.saturating_sub(gear_height) / 2);
    let mut scene = GraphicsScene::empty();
    let mut gears = [clip; CANONICAL_PATCHBAY_GEARS.len()];
    for (index, identity) in CANONICAL_PATCHBAY_GEARS.iter().enumerate() {
        let x = region.x.saturating_add(inset_x).saturating_add(
            u16::try_from(index)
                .unwrap_or(u16::MAX)
                .saturating_mul(gear_width.saturating_add(16)),
        );
        let bounds = LayoutRect {
            x: i16::try_from(x).map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?,
            y: i16::try_from(gear_y).map_err(|_| TourWorkspaceSceneRefusal::MissingRegion)?,
            width: gear_width,
            height: gear_height,
        };
        gears[index] = bounds;
        let paint = if state.selected_patchbay_subject.as_deref() == Some(*identity) {
            GraphicsPaintRole::Accent
        } else {
            GraphicsPaintRole::Foreground
        };
        scene
            .push(
                GraphicsCommand::rect(bounds, clip, paint, GraphicsShapeStyle::Stroke)
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
        let label = identity.rsplit('/').next().unwrap_or(identity);
        scene
            .push(
                GraphicsCommand::text(inset(bounds), clip, paint, label)
                    .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    }
    for pair in gears.windows(2) {
        let left = pair[0];
        let right = pair[1];
        let x = left.x.saturating_add_unsigned(left.width);
        let width = u16::try_from(i32::from(right.x).saturating_sub(i32::from(x)))
            .unwrap_or_default()
            .max(1);
        let cord = LayoutRect {
            x,
            y: left
                .y
                .saturating_add_unsigned(left.height.saturating_div(2)),
            width,
            height: 2,
        };
        scene
            .push(
                GraphicsCommand::rect(
                    cord,
                    clip,
                    GraphicsPaintRole::Accent,
                    GraphicsShapeStyle::Fill,
                )
                .map_err(TourWorkspaceSceneRefusal::Graphics)?,
            )
            .map_err(TourWorkspaceSceneRefusal::Graphics)?;
    }
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
        assert_eq!(scene.commands().len(), 8);
        let frames: alloc::vec::Vec<_> = scene
            .commands()
            .iter()
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
        assert!(
            scene.commands()[5]
                .payload()
                .starts_with("form meet-one-gear")
        );
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
    fn patchbay_composition_draws_three_semantic_gears_and_two_cords() {
        let mut state = TourWorkspaceState::canonical(12, TourWorkspacePhase::PatchbayOpen);
        state.selected_patchbay_subject = Some(CANONICAL_PATCHBAY_GEARS[1].into());
        let composition = composition_for_state(640, 480, &state).unwrap();
        assert_eq!(composition.layers().len(), MAX_TOUR_COMPOSITION_LAYERS);
        let patchbay = &composition.layers()[1];
        assert_eq!(patchbay.commands().len(), 8);
        assert_eq!(
            patchbay
                .commands()
                .iter()
                .filter(|command| command.kind == GraphicsCommandKind::Text)
                .map(|command| command.payload())
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec!["words", "change", "result"]
        );
        let gear_frames = patchbay
            .commands()
            .iter()
            .filter(|command| {
                command.kind == GraphicsCommandKind::Rect
                    && command.style == GraphicsShapeStyle::Stroke
            })
            .collect::<alloc::vec::Vec<_>>();
        assert_eq!(gear_frames.len(), 3);
        assert_eq!(gear_frames[1].paint, GraphicsPaintRole::Accent);
        assert!(patchbay.commands().iter().all(|command| {
            command.clip_class() == conduit_presentation::GraphicsClipClass::FullyVisible
        }));
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
}
