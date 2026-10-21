//! Original fixed 24×24 native drawings for the semantic icon vocabulary.
//! Geometry is rasterized directly from bounded compile-time shape rules.
use super::{put, DisplayError, DisplayReceipt, PixelTarget};
use conduit_face::{FaceIconKey as Icon, GraphicsCommand, LayoutRect};

const EDGE: i32 = 24;

pub(super) fn draw(
    target: &mut impl PixelTarget,
    command: &GraphicsCommand,
    clip: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let icon = Icon::from_token(command.payload()).ok_or(DisplayError::InvalidExtent)?;
    let width = i32::from(command.bounds.width);
    let height = i32::from(command.bounds.height);
    if width <= 0 || height <= 0 {
        return Err(DisplayError::InvalidExtent);
    }
    for y in 0..height {
        for x in 0..width {
            let source_x = x * EDGE / width;
            let source_y = y * EDGE / height;
            let px = i32::from(command.bounds.x) + x;
            let py = i32::from(command.bounds.y) + y;
            if coverage::ink(icon, i64::from(source_x) * 8, i64::from(source_y) * 8)
                && px >= i32::from(clip.x)
                && py >= i32::from(clip.y)
                && px < i32::from(clip.x) + i32::from(clip.width)
                && py < i32::from(clip.y) + i32::from(clip.height)
            {
                put(target, px as u32, py as u32, color, receipt)?;
            }
        }
    }
    Ok(())
}

#[path = "icons/coverage.rs"]
mod coverage;
pub(super) fn draw_smooth(
    target: &mut impl super::RetainedPixelTarget,
    command: &GraphicsCommand,
    clip: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let icon = Icon::from_token(command.payload()).ok_or(DisplayError::InvalidExtent)?;
    let width = i32::from(command.bounds.width);
    let height = i32::from(command.bounds.height);
    if width == 0 || height == 0 {
        return Err(DisplayError::InvalidExtent);
    }
    super::coverage::raster(target, command.bounds, clip, color, receipt, |x, y| {
        coverage::ink(
            icon,
            i64::from(x * EDGE / width - 4),
            i64::from(y * EDGE / height - 4),
        )
    })
}
