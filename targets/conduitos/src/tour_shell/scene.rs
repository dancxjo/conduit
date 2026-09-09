use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsSymbol,
    LayoutRect, Presentation,
};

use super::TourShellError;

pub(super) const SCROLL_CONTENT_HEIGHT: u16 = 768;

#[cfg(test)]
#[path = "scene_content_tests.rs"]
mod content_tests;

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
        let status_height = crate::tour_workspace::STATUS_HEIGHT;
        let inspector_width = crate::tour_workspace::inspector_width(width);
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
    symbol: GraphicsSymbol,
) -> Result<GraphicsScene, TourShellError> {
    let inset = crate::display::style::NATIVE_STYLE.panel_inset;
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
    let heading_paint = match symbol {
        GraphicsSymbol::Warning => GraphicsPaintRole::Warning,
        GraphicsSymbol::Failure => GraphicsPaintRole::Danger,
        GraphicsSymbol::Success => GraphicsPaintRole::Success,
        _ => GraphicsPaintRole::Accent,
    };
    let symbol_space = if cfg!(feature = "native-compositor") {
        scene
            .push(
                GraphicsCommand::symbol(
                    LayoutRect {
                        x: inset as i16,
                        y: inset as i16,
                        width: 24,
                        height: 24,
                    },
                    local,
                    heading_paint,
                    symbol,
                )
                .map_err(|_| TourShellError::Scene)?,
            )
            .map_err(|_| TourShellError::Scene)?;
        24 + crate::display::style::NATIVE_STYLE.inset
    } else {
        0
    };
    let title_bounds = LayoutRect {
        x: (inset + symbol_space) as i16,
        y: inset as i16,
        width: local.width.saturating_sub(2 * inset + symbol_space).max(1),
        height: 24,
    };
    scene
        .push(
            GraphicsCommand::text(title_bounds, local, heading_paint, title)
                .and_then(|command| {
                    command.with_text_role(conduit_presentation::GraphicsTextRole::Heading)
                })
                .map_err(|_| TourShellError::Scene)?,
        )
        .map_err(|_| TourShellError::Scene)?;
    if detail.is_empty() {
        return Ok(scene);
    }
    let detail_bounds = LayoutRect {
        x: inset as i16,
        y: 38,
        width: local.width.saturating_sub(2 * inset),
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
    for (index, (key, symbol)) in [
        ("body", GraphicsSymbol::Body),
        ("wake", GraphicsSymbol::Wake),
        ("plan", GraphicsSymbol::Plan),
        ("play", GraphicsSymbol::Play),
        ("lines", GraphicsSymbol::Line),
        ("host", GraphicsSymbol::Host),
    ]
    .into_iter()
    .enumerate()
    {
        let identity = alloc::format!("tour/status/{key}");
        let subject = presentation
            .subjects
            .iter()
            .find(|subject| subject.identity == identity)
            .ok_or(TourShellError::Identity)?;
        let item = presentation
            .text
            .iter()
            .find(|item| item.subject == identity)
            .ok_or(TourShellError::Identity)?;
        let column = bounds.width / 6;
        let cell = LayoutRect {
            x: i16::try_from(index as u16 * column).map_err(|_| TourShellError::Identity)?,
            y: 0,
            width: column,
            height: bounds.height,
        };
        // Narrow/rescue surfaces retain the complete textual status. Icons are
        // redundant graphical cues selected from exact status identities above.
        let icon_space = if cfg!(feature = "native-compositor") && column >= 160 {
            let style = crate::display::style::NATIVE_STYLE;
            let icon = LayoutRect {
                x: cell.x + style.inset as i16,
                y: style.panel_inset as i16,
                width: 24,
                height: 24,
            };
            scene
                .push(
                    GraphicsCommand::symbol(icon, cell, GraphicsPaintRole::Accent, symbol)
                        .map_err(|_| TourShellError::Scene)?,
                )
                .map_err(|_| TourShellError::Scene)?;
            24 + style.inset
        } else {
            0
        };
        let text_bounds = LayoutRect {
            x: cell.x + (crate::display::style::NATIVE_STYLE.inset + icon_space) as i16,
            y: crate::display::style::NATIVE_STYLE.panel_inset as i16,
            width: column
                .saturating_sub(2 * crate::display::style::NATIVE_STYLE.inset + icon_space)
                .max(1),
            height: bounds
                .height
                .saturating_sub(crate::display::style::NATIVE_STYLE.panel_inset)
                .max(1),
        };
        let text = alloc::format!("{}\n{}", subject.label, item.text);
        scene
            .push(
                GraphicsCommand::text(text_bounds, cell, GraphicsPaintRole::Foreground, &text)
                    .and_then(|command| {
                        command.with_text_role(conduit_presentation::GraphicsTextRole::Label)
                    })
                    .map_err(|_| TourShellError::Scene)?,
            )
            .map_err(|_| TourShellError::Scene)?;
    }
    Ok(scene)
}
pub(super) fn inspector_scene(
    bounds: LayoutRect,
    presentation: &Presentation,
    scroll_y: u16,
) -> Result<GraphicsScene, TourShellError> {
    if scroll_y > super::scroll::MAX_SCROLL_CONTENT_HEIGHT {
        return Err(TourShellError::Identity);
    }
    let mut scene = panel_scene(bounds, "INSPECTOR", "", GraphicsSymbol::Inspect)?;
    if let Some(action) = presentation.subjects.iter().find(|subject| {
        subject.identity == conduit_tour_model::INSPECTOR_CLOSE_ACTION_ID
            && subject.role == conduit_presentation::PresentationRole::Action
    }) {
        let button = super::controls::inspector_close_bounds(bounds.width);
        scene
            .push(
                GraphicsCommand::rect(
                    button,
                    button,
                    GraphicsPaintRole::Accent,
                    crate::display::style::panel_outline(),
                )
                .map_err(|_| TourShellError::Scene)?,
            )
            .map_err(|_| TourShellError::Scene)?;
        let text = LayoutRect {
            x: button.x + 8,
            y: button.y + 4,
            width: button.width - 16,
            height: button.height - 8,
        };
        scene
            .push(
                GraphicsCommand::text(text, button, GraphicsPaintRole::Foreground, &action.label)
                    .map_err(|_| TourShellError::Scene)?,
            )
            .map_err(|_| TourShellError::Scene)?;
    }
    super::fields::project(bounds, presentation, scroll_y, Some(&mut scene))?;
    Ok(scene)
}
pub(super) fn transient_scene(
    bounds: LayoutRect,
    presentation: &Presentation,
    scroll_y: u16,
) -> Result<GraphicsScene, TourShellError> {
    if presentation.subjects.iter().any(|subject| {
        subject.identity == conduit_tour_model::TourTransientKind::Chooser.subject_identity()
    }) {
        return chooser_scene(bounds, presentation, scroll_y);
    }
    use conduit_tour_model::TourTransientKind;
    let (subject, symbol) = [
        (TourTransientKind::Refusal, GraphicsSymbol::Warning),
        // A confirmation surface is not proof of successful execution.
        (TourTransientKind::Confirmation, GraphicsSymbol::Info),
    ]
    .into_iter()
    .find_map(|(kind, symbol)| {
        presentation
            .subjects
            .iter()
            .find(|subject| subject.identity == kind.subject_identity())
            .map(|subject| (subject, symbol))
    })
    .ok_or(TourShellError::Identity)?;
    scroll_scene(
        bounds,
        &subject.label,
        first_text(presentation),
        scroll_y,
        symbol,
    )
}

fn chooser_scene(
    bounds: LayoutRect,
    presentation: &Presentation,
    scroll_y: u16,
) -> Result<GraphicsScene, TourShellError> {
    if scroll_y > SCROLL_CONTENT_HEIGHT {
        return Err(TourShellError::Identity);
    }
    let mut scene = panel_scene(
        bounds,
        "CHOOSE GEAR",
        first_text(presentation),
        GraphicsSymbol::Gear,
    )?;
    let viewport = LayoutRect {
        x: 0,
        y: 70,
        width: bounds.width,
        height: bounds.height.saturating_sub(70).max(1),
    };
    for (index, gear) in conduit_tour_model::CANONICAL_PATCHBAY_GEARS
        .into_iter()
        .enumerate()
    {
        let y = 76 + index as i32 * 140 - i32::from(scroll_y);
        if y + 60 <= 70 || y >= i32::from(bounds.height) {
            continue;
        }
        let row = LayoutRect {
            x: 12,
            y: i16::try_from(y).map_err(|_| TourShellError::Identity)?,
            width: bounds.width.saturating_sub(24).max(1),
            height: 60,
        };
        scene
            .push(
                GraphicsCommand::rect(
                    row,
                    viewport,
                    GraphicsPaintRole::Accent,
                    crate::display::style::panel_outline(),
                )
                .map_err(|_| TourShellError::Scene)?,
            )
            .map_err(|_| TourShellError::Scene)?;
        let label = alloc::format!("Select {gear}");
        let text = LayoutRect {
            x: row.x + 8,
            y: row.y + 8,
            width: row.width.saturating_sub(16).max(1),
            height: 44,
        };
        scene
            .push(
                GraphicsCommand::text(text, viewport, GraphicsPaintRole::Foreground, &label)
                    .map_err(|_| TourShellError::Scene)?,
            )
            .map_err(|_| TourShellError::Scene)?;
    }
    Ok(scene)
}

fn scroll_scene(
    bounds: LayoutRect,
    title: &str,
    detail: &str,
    scroll_y: u16,
    symbol: GraphicsSymbol,
) -> Result<GraphicsScene, TourShellError> {
    if scroll_y > SCROLL_CONTENT_HEIGHT {
        return Err(TourShellError::Identity);
    }
    let viewport = LayoutRect {
        x: 0,
        y: 0,
        width: bounds.width,
        height: bounds.height,
    };
    let mut scene = panel_scene(bounds, title, detail, symbol)?;
    for row in 0..5_u16 {
        let content_y = 76_u16
            .checked_add(row.checked_mul(140).ok_or(TourShellError::Identity)?)
            .ok_or(TourShellError::Identity)?;
        let visible_y = i32::from(content_y) - i32::from(scroll_y);
        let Ok(y) = i16::try_from(visible_y) else {
            continue;
        };
        let row_bounds = LayoutRect {
            x: 12,
            y,
            width: viewport.width.saturating_sub(24),
            height: 60,
        };
        scene
            .push(
                GraphicsCommand::rect(
                    row_bounds,
                    viewport,
                    if row % 2 == 0 {
                        GraphicsPaintRole::Status
                    } else {
                        GraphicsPaintRole::Background
                    },
                    GraphicsShapeStyle::Fill,
                )
                .map_err(|_| TourShellError::Scene)?,
            )
            .map_err(|_| TourShellError::Scene)?;
    }
    Ok(scene)
}
