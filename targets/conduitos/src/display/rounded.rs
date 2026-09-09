//! Rounded native borders; clipping never creates an artificial edge.
use super::{DisplayError, DisplayReceipt, PixelTarget, profile, put};
use conduit_presentation::LayoutRect;

pub(super) fn stroke(
    target: &mut impl PixelTarget,
    rect: LayoutRect,
    clip: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let width = i32::from(rect.width);
    let height = i32::from(rect.height);
    let border = i32::from(profile::BORDER);
    let radius = i32::from(profile::RADIUS).min(width / 2).min(height / 2);
    for y in i32::from(clip.y)..i32::from(clip.y) + i32::from(clip.height) {
        for x in i32::from(clip.x)..i32::from(clip.x) + i32::from(clip.width) {
            let dx = x - i32::from(rect.x);
            let dy = y - i32::from(rect.y);
            if inside(dx, dy, width, height, radius)
                && !inside(
                    dx - border,
                    dy - border,
                    width - 2 * border,
                    height - 2 * border,
                    (radius - border).max(0),
                )
            {
                put(target, x as u32, y as u32, color, receipt)?;
            }
        }
    }
    Ok(())
}

fn inside(x: i32, y: i32, width: i32, height: i32, radius: i32) -> bool {
    if x < 0 || y < 0 || x >= width || y >= height {
        return false;
    }
    let dx = if x < radius {
        radius - x
    } else {
        (x - (width - 1 - radius)).max(0)
    };
    let dy = if y < radius {
        radius - y
    } else {
        (y - (height - 1 - radius)).max(0)
    };
    dx * dx + dy * dy <= radius * radius
}
