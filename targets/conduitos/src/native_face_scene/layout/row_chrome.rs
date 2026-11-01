//! Paint chrome from admitted row geometry; no layout or input authority.
use super::*;

pub(in crate::native_face_scene) struct RowChromeMetrics {
    pub focus_marker_width: u16,
    pub control_radius: Option<u8>,
}

/// Draw panel surfaces before any rows: authored geometry order may differ
/// from the source reading order restored after layout admission.
pub(in crate::native_face_scene) fn append_panel_chrome(
    scene: &mut GraphicsScene,
    row: &Row,
    screen: LayoutRect,
    radius: Option<u8>,
) -> Result<(), FaceSceneError> {
    if let Some(bounds) = row.panel_bounds {
        for (paint, shape) in [
            (GraphicsPaintRole::Surface, GraphicsShapeStyle::RoundedFill),
            (GraphicsPaintRole::Border, GraphicsShapeStyle::RoundedStroke),
        ] {
            push(
                scene,
                GraphicsCommand::rect(bounds, screen, paint, shape)
                    .and_then(|command| corners::apply(command, radius)),
            )?;
        }
    }
    Ok(())
}

/// Row-local drawing belongs with its measured bounds. A heading marker and
/// control outline are Mask geometry; neither changes the Face or hit target.
pub(in crate::native_face_scene) fn append_row_chrome(
    scene: &mut GraphicsScene,
    row: &Row,
    screen: LayoutRect,
    outline: GraphicsPaintRole,
    showing_details: bool,
    focused: bool,
    metrics: RowChromeMetrics,
) -> Result<(), FaceSceneError> {
    let RowChromeMetrics {
        focus_marker_width,
        control_radius,
    } = metrics;
    if !showing_details && row.region_start && row.region.is_none() {
        push(
            scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: row.bounds.x,
                    y: row.bounds.y,
                    width: row.bounds.width,
                    height: 2,
                },
                screen,
                GraphicsPaintRole::Muted,
                GraphicsShapeStyle::Fill,
            ),
        )?;
    }
    if row.control.is_some() && row.checkbox.is_none() {
        push(
            scene,
            GraphicsCommand::rect(
                row.bounds,
                screen,
                if row.emphasized() {
                    GraphicsPaintRole::Accent
                } else if focused || row.paint == GraphicsPaintRole::Muted {
                    outline
                } else {
                    GraphicsPaintRole::Border
                },
                if row.emphasized() {
                    GraphicsShapeStyle::RoundedFill
                } else {
                    GraphicsShapeStyle::RoundedStroke
                },
            )
            .and_then(|command| corners::apply(command, control_radius)),
        )?;
    }
    // An inset outline distinguishes focus even with identical paints. Choice
    // indicators remain independent; other controls also have their outer box.
    if focused {
        // Selected control padding may be zero. Keep the non-color marker in
        // its admitted leading gutter, outside both glyph and choice ink.
        let (marker, shape) = if row.insets.leading() != 0 {
            (
                LayoutRect {
                    x: row.bounds.x + ((row.insets.leading() - focus_marker_width) / 2) as i16,
                    y: row.bounds.y + 3,
                    width: focus_marker_width,
                    height: row.bounds.height - 6,
                },
                GraphicsShapeStyle::Fill,
            )
        } else {
            (
                LayoutRect {
                    x: row.bounds.x + 3,
                    y: row.bounds.y + 3,
                    width: row.bounds.width - 6,
                    height: row.bounds.height - 6,
                },
                GraphicsShapeStyle::Stroke,
            )
        };
        push(
            scene,
            GraphicsCommand::rect(
                marker,
                screen,
                // Filled primary actions need an interior contrast against
                // Accent, including white fills under forced colors. Keep the
                // outer focus outline, but use the same contrasting paint as
                // the primary label for this independent inset marker.
                if row.emphasized() {
                    GraphicsPaintRole::Background
                } else {
                    outline
                },
                shape,
            ),
        )?;
    }
    if !showing_details
        && (row.icon.is_none() || row.emphasized())
        && matches!(
            row.role,
            GraphicsTextRole::Title | GraphicsTextRole::Heading
        )
    {
        if row.emphasized() {
            push(
                scene,
                GraphicsCommand::rect(
                    row.bounds,
                    screen,
                    if focused {
                        outline
                    } else {
                        GraphicsPaintRole::Border
                    },
                    GraphicsShapeStyle::RoundedStroke,
                )
                .and_then(|command| corners::apply(command, control_radius)),
            )?;
            return Ok(());
        }
        push(
            scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: row.bounds.x,
                    y: row.bounds.y + 4,
                    width: 3,
                    height: row.bounds.height - 8,
                },
                screen,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Fill,
            ),
        )?;
    }
    Ok(())
}
