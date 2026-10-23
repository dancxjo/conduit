//! Graphical retained-scene rendering; rescue scanout rendering stays separate.

use super::{TextRole, Typography, render_text_selected};
use crate::display::{DisplayError, DisplayReceipt, RetainedPixelTarget};
use conduit_face::{GraphicsCommand, GraphicsCommandKind, GraphicsPaintRole, GraphicsScene};

/// Resolve text purpose in the Presenter without altering portable text meaning.
/// The caller's role selector must be deterministic for the admitted scene.
pub fn render_scene(
    target: &mut impl RetainedPixelTarget,
    scene: &GraphicsScene,
    role: impl Fn(usize, &GraphicsCommand) -> TextRole,
) -> Result<DisplayReceipt, DisplayError> {
    render_scene_scaled(target, scene, role, 100)
}
pub fn render_scene_scaled(
    target: &mut impl RetainedPixelTarget,
    scene: &GraphicsScene,
    role: impl Fn(usize, &GraphicsCommand) -> TextRole,
    text_scale_percent: u16,
) -> Result<DisplayReceipt, DisplayError> {
    let format = target.format().validate()?;
    render_scene_configured(target, scene, role, text_scale_percent, |paint| {
        super::super::paint(format, paint)
    })
}

/// Resolve the admitted palette per render; Host accessibility policy belongs
/// to the supplied resolver and never changes Face or command identity.
pub fn render_scene_configured(
    target: &mut impl RetainedPixelTarget,
    scene: &GraphicsScene,
    role: impl Fn(usize, &GraphicsCommand) -> TextRole,
    text_scale_percent: u16,
    resolve_paint: impl Fn(GraphicsPaintRole) -> u32,
) -> Result<DisplayReceipt, DisplayError> {
    render_scene_selected(
        target,
        scene,
        role,
        text_scale_percent,
        Typography::EDITORIAL,
        resolve_paint,
    )
}

pub fn render_scene_selected(
    target: &mut impl RetainedPixelTarget,
    scene: &GraphicsScene,
    role: impl Fn(usize, &GraphicsCommand) -> TextRole,
    text_scale_percent: u16,
    typography: Typography,
    resolve_paint: impl Fn(GraphicsPaintRole) -> u32,
) -> Result<DisplayReceipt, DisplayError> {
    super::TextScale::new(text_scale_percent)?;
    let format = target.format().validate()?;
    let mut receipt = DisplayReceipt::default();
    for (index, command) in scene.commands().iter().enumerate() {
        if command.kind == GraphicsCommandKind::Text {
            let text = render_text_selected(
                target,
                command.payload(),
                typography.style(role(index, command)),
                command.bounds,
                command.clip,
                resolve_paint(command.paint),
                text_scale_percent,
            )?;
            receipt.pixels_written = receipt
                .pixels_written
                .checked_add(text.pixels_written)
                .ok_or(DisplayError::InvalidExtent)?;
        } else {
            super::super::coverage::render_command(
                target,
                format,
                command,
                &mut receipt,
                resolve_paint(command.paint),
            )?;
        }
        receipt.commands += 1;
    }
    Ok(receipt)
}
