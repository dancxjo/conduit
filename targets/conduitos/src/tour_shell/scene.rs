use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect, Presentation,
};

use super::TourShellError;

pub(super) struct ShellLayout {
    pub(super) workspace: LayoutRect,
    pub(super) inspector: LayoutRect,
    pub(super) status: LayoutRect,
    pub(super) transient: LayoutRect,
}

impl ShellLayout {
    pub(super) fn new(width: u16, height: u16) -> Result<Self, TourShellError> {
        if width < 320 || height < 240 {
            return Err(TourShellError::Identity);
        }
        let status_height = 64;
        let inspector_width = (width / 3).max(180);
        let content_height = height
            .checked_sub(status_height)
            .ok_or(TourShellError::Identity)?;
        Ok(Self {
            workspace: LayoutRect {
                x: 0,
                y: 0,
                width,
                height,
            },
            inspector: LayoutRect {
                x: i16::try_from(width - inspector_width).map_err(|_| TourShellError::Identity)?,
                y: 0,
                width: inspector_width,
                height: content_height,
            },
            status: LayoutRect {
                x: 0,
                y: i16::try_from(content_height).map_err(|_| TourShellError::Identity)?,
                width,
                height: status_height,
            },
            transient: LayoutRect {
                x: i16::try_from(width / 4).map_err(|_| TourShellError::Identity)?,
                y: i16::try_from(height / 3).map_err(|_| TourShellError::Identity)?,
                width: width / 2,
                height: height / 3,
            },
        })
    }
}

fn panel_scene(
    bounds: LayoutRect,
    title: &str,
    detail: &str,
) -> Result<GraphicsScene, TourShellError> {
    let local = LayoutRect {
        x: 0,
        y: 0,
        width: bounds.width,
        height: bounds.height,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                local,
                local,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .map_err(|_| TourShellError::Scene)?,
        )
        .map_err(|_| TourShellError::Scene)?;
    let title_bounds = LayoutRect {
        x: 12,
        y: 12,
        width: local.width.saturating_sub(24),
        height: 18,
    };
    scene
        .push(
            GraphicsCommand::text(title_bounds, local, GraphicsPaintRole::Accent, title)
                .map_err(|_| TourShellError::Scene)?,
        )
        .map_err(|_| TourShellError::Scene)?;
    let detail_bounds = LayoutRect {
        x: 12,
        y: 38,
        width: local.width.saturating_sub(24),
        height: local.height.saturating_sub(48).max(1),
    };
    scene
        .push(
            GraphicsCommand::text(detail_bounds, local, GraphicsPaintRole::Foreground, detail)
                .map_err(|_| TourShellError::Scene)?,
        )
        .map_err(|_| TourShellError::Scene)?;
    Ok(scene)
}

fn first_text(presentation: &Presentation) -> &str {
    presentation
        .text
        .first()
        .map_or("", |text| text.text.as_str())
}

pub(super) fn status_scene(
    bounds: LayoutRect,
    presentation: &Presentation,
) -> Result<GraphicsScene, TourShellError> {
    panel_scene(
        bounds,
        "BODY / WAKE / PLAN / PLAY",
        first_text(presentation),
    )
}
pub(super) fn inspector_scene(
    bounds: LayoutRect,
    presentation: &Presentation,
) -> Result<GraphicsScene, TourShellError> {
    panel_scene(bounds, "GEAR BACK", first_text(presentation))
}
pub(super) fn transient_scene(
    bounds: LayoutRect,
    presentation: &Presentation,
) -> Result<GraphicsScene, TourShellError> {
    panel_scene(bounds, "DETAIL", first_text(presentation))
}
