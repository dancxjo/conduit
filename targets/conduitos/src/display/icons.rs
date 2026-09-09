//! Original finite 16×16 shell icon vocabulary, independent of font coverage.
use super::{DisplayError, DisplayReceipt, PixelTarget, put};
use conduit_presentation::{GraphicsCommand, LayoutRect, PresentationIconKey as Icon};

pub(super) fn draw(
    target: &mut impl PixelTarget,
    command: &GraphicsCommand,
    clip: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let icon = Icon::from_token(command.payload()).ok_or(DisplayError::InvalidExtent)?;
    for y in 0..16_i32 {
        for x in 0..16_i32 {
            let px = i32::from(command.bounds.x) + x;
            let py = i32::from(command.bounds.y) + y;
            if ink(icon, x, y)
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

fn box_edge(x: i32, y: i32, left: i32, top: i32, right: i32, bottom: i32) -> bool {
    x >= left
        && x <= right
        && y >= top
        && y <= bottom
        && (x == left || x == right || y == top || y == bottom)
}

fn ink(icon: Icon, x: i32, y: i32) -> bool {
    let distance = (x - 7) * (x - 7) + (y - 7) * (y - 7);
    match icon {
        Icon::Body => box_edge(x, y, 2, 2, 12, 12) || box_edge(x, y, 5, 5, 9, 9),
        Icon::Wake | Icon::Clock => {
            (30..=42).contains(&distance)
                || (x == 7 && (3..=7).contains(&y))
                || (y == 7 && (7..=10).contains(&x))
        }
        Icon::Plan => {
            box_edge(x, y, 3, 1, 12, 14) || ((y == 5 || y == 8 || y == 11) && (5..=10).contains(&x))
        }
        Icon::Play | Icon::Confirm => (4..=12).contains(&x) && (y - 7).abs() <= (12 - x) / 2,
        Icon::Host | Icon::Presentation => {
            box_edge(x, y, 1, 2, 14, 10)
                || (x == 7 && (11..=13).contains(&y))
                || (y == 13 && (4..=10).contains(&x))
        }
        Icon::Port => (8..=16).contains(&distance),
        Icon::Line => {
            (x == 3 && (3..=8).contains(&y))
                || (y == 8 && (3..=12).contains(&x))
                || (x == 12 && (8..=13).contains(&y))
        }
        Icon::Status => {
            ((2..=6).contains(&x) && y == x + 4) || ((6..=13).contains(&x) && y == 16 - x)
        }
        Icon::Warning => {
            y == 13 && (1..=13).contains(&x)
                || (2..=13).contains(&y) && (x == 7 - (y - 1) / 2 || x == 7 + (y - 1) / 2)
                || x == 7 && ((6..=9).contains(&y) || y == 11)
        }
        Icon::Close => (3..=12).contains(&x) && (y == x || y == 15 - x),
        Icon::Back => {
            (y == 7 && (2..=13).contains(&x))
                || ((2..=7).contains(&x) && (y == 9 - x || y == x + 5))
        }
        _ => {
            (17..=30).contains(&distance)
                || ((x == 7 || y == 7) && (2..=12).contains(&x) && (2..=12).contains(&y))
        }
    }
}
