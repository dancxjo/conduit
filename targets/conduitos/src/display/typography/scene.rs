//! Graphical retained-scene rendering; rescue scanout rendering stays separate.

use super::{TextRole, render_text};
use crate::display::{DisplayError, DisplayReceipt, RetainedPixelTarget};
use conduit_presentation::{GraphicsCommand, GraphicsCommandKind, GraphicsScene};

/// Resolve text purpose in the Presenter without altering portable text meaning.
/// The caller's role selector must be deterministic for the admitted scene.
pub fn render_scene(
    target: &mut impl RetainedPixelTarget,
    scene: &GraphicsScene,
    role: impl Fn(usize, &GraphicsCommand) -> TextRole,
) -> Result<DisplayReceipt, DisplayError> {
    let format = target.format().validate()?;
    let mut receipt = DisplayReceipt::default();
    for (index, command) in scene.commands().iter().enumerate() {
        if command.kind == GraphicsCommandKind::Text {
            let text = render_text(
                target,
                command.payload(),
                role(index, command),
                command.bounds,
                command.clip,
                super::super::paint(format, command.paint),
            )?;
            receipt.pixels_written = receipt
                .pixels_written
                .checked_add(text.pixels_written)
                .ok_or(DisplayError::InvalidExtent)?;
        } else {
            super::super::render_command(target, format, command, &mut receipt)?;
        }
        receipt.commands += 1;
    }
    Ok(receipt)
}
