//! Original fixed 24×24 native drawings for the semantic icon vocabulary.
//! Geometry is rasterized directly from bounded compile-time shape rules.
use super::{DisplayError, DisplayReceipt, PixelTarget, put};
use conduit_presentation::{GraphicsCommand, LayoutRect, PresentationIconKey as Icon};

const EDGE: i32 = 24;

pub(super) fn draw(
    target: &mut impl PixelTarget,
    command: &GraphicsCommand,
    clip: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let icon = Icon::from_token(command.payload()).ok_or(DisplayError::InvalidExtent)?;
    for y in 0..EDGE {
        for x in 0..EDGE {
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

const fn line(x: i32, y: i32, ax: i32, ay: i32, bx: i32, by: i32) -> bool {
    let dx = bx - ax;
    let dy = by - ay;
    let px = x - ax;
    let py = y - ay;
    let length = dx * dx + dy * dy;
    let dot = px * dx + py * dy;
    let cross = px * dy - py * dx;
    dot >= 0 && dot <= length && cross * cross <= length
}

const fn ring(x: i32, y: i32, cx: i32, cy: i32, radius: i32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    let distance = dx * dx + dy * dy;
    distance >= (radius - 1) * (radius - 1) && distance <= (radius + 1) * (radius + 1)
}

const fn frame(x: i32, y: i32, left: i32, top: i32, right: i32, bottom: i32) -> bool {
    line(x, y, left, top, right, top)
        || line(x, y, right, top, right, bottom)
        || line(x, y, right, bottom, left, bottom)
        || line(x, y, left, bottom, left, top)
}

const fn letter_a(x: i32, y: i32) -> bool {
    line(x, y, 5, 20, 12, 3) || line(x, y, 12, 3, 19, 20) || line(x, y, 8, 14, 16, 14)
}

const fn ink(icon: Icon, x: i32, y: i32) -> bool {
    use Icon::*;
    match icon {
        Clock => ring(x, y, 12, 12, 9)
            || line(x, y, 12, 6, 12, 12)
            || line(x, y, 12, 12, 17, 15),
        Repeat2 => {
            line(x, y, 5, 6, 19, 6)
                || line(x, y, 19, 6, 19, 11)
                || line(x, y, 19, 6, 16, 3)
                || line(x, y, 5, 18, 19, 18)
                || line(x, y, 5, 13, 5, 18)
                || line(x, y, 5, 18, 8, 21)
        }
        Presentation => {
            frame(x, y, 3, 3, 21, 16)
                || line(x, y, 12, 16, 12, 21)
                || line(x, y, 7, 21, 17, 21)
        }
        Type => line(x, y, 4, 4, 20, 4)
            || line(x, y, 12, 4, 12, 20)
            || line(x, y, 8, 20, 16, 20),
        CaseUpper => letter_a(x, y),
        Combine => {
            line(x, y, 3, 5, 8, 5)
                || line(x, y, 8, 5, 13, 12)
                || line(x, y, 3, 19, 8, 19)
                || line(x, y, 8, 19, 13, 12)
                || line(x, y, 13, 12, 21, 12)
                || line(x, y, 17, 8, 21, 12)
                || line(x, y, 17, 16, 21, 12)
        }
        Tally5 => {
            line(x, y, 4, 4, 4, 20)
                || line(x, y, 9, 4, 9, 20)
                || line(x, y, 14, 4, 14, 20)
                || line(x, y, 19, 4, 19, 20)
                || line(x, y, 2, 17, 21, 7)
        }
        ChartColumnsIncreasing => {
            frame(x, y, 3, 15, 7, 21)
                || frame(x, y, 10, 10, 14, 21)
                || frame(x, y, 17, 3, 21, 21)
        }
        FileOutput => {
            line(x, y, 4, 3, 14, 3)
                || line(x, y, 4, 3, 4, 21)
                || line(x, y, 4, 21, 14, 21)
                || line(x, y, 14, 3, 18, 7)
                || line(x, y, 18, 7, 18, 9)
                || line(x, y, 10, 14, 22, 14)
                || line(x, y, 18, 10, 22, 14)
                || line(x, y, 18, 18, 22, 14)
        }
        Keyboard => {
            frame(x, y, 2, 5, 22, 19)
                || line(x, y, 7, 16, 17, 16)
                || (y >= 8 && y <= 12 && y % 4 == 0 && x >= 5 && x <= 19 && x % 4 == 1)
        }
        GenericGear => {
            ring(x, y, 12, 12, 8)
                || ring(x, y, 12, 12, 3)
                || line(x, y, 12, 1, 12, 5)
                || line(x, y, 12, 19, 12, 23)
                || line(x, y, 1, 12, 5, 12)
                || line(x, y, 19, 12, 23, 12)
        }
        Body => ring(x, y, 12, 6, 3) || frame(x, y, 5, 13, 19, 21),
        Wake => {
            ring(x, y, 12, 12, 6)
                || line(x, y, 12, 1, 12, 4)
                || line(x, y, 12, 20, 12, 23)
                || line(x, y, 1, 12, 4, 12)
                || line(x, y, 20, 12, 23, 12)
        }
        Plan => {
            frame(x, y, 3, 3, 8, 8)
                || frame(x, y, 16, 16, 21, 21)
                || line(x, y, 8, 6, 18, 6)
                || line(x, y, 18, 6, 18, 16)
        }
        Play => x >= 6 && x <= 20 && y >= 3 && y <= 21 && (y - 12).abs() * 14 <= (20 - x) * 9,
        Host => {
            frame(x, y, 3, 3, 21, 16)
                || line(x, y, 12, 16, 12, 21)
                || line(x, y, 7, 21, 17, 21)
        }
        Port => {
            ring(x, y, 8, 12, 5)
                || line(x, y, 14, 12, 22, 12)
                || line(x, y, 22, 12, 18, 8)
                || line(x, y, 22, 12, 18, 16)
        }
        Line => {
            frame(x, y, 1, 13, 7, 19)
                || frame(x, y, 17, 5, 23, 11)
                || line(x, y, 7, 16, 17, 8)
        }
        Status => {
            ring(x, y, 12, 12, 10)
                || line(x, y, 12, 10, 12, 18)
                || ring(x, y, 12, 6, 1)
        }
        Warning => {
            line(x, y, 12, 2, 2, 21)
                || line(x, y, 2, 21, 22, 21)
                || line(x, y, 22, 21, 12, 2)
                || line(x, y, 12, 8, 12, 14)
                || ring(x, y, 12, 18, 1)
        }
        Close => line(x, y, 5, 5, 19, 19) || line(x, y, 19, 5, 5, 19),
        Back => line(x, y, 3, 12, 21, 12)
            || line(x, y, 3, 12, 10, 5)
            || line(x, y, 3, 12, 10, 19),
        Confirm => line(x, y, 3, 12, 9, 18) || line(x, y, 9, 18, 21, 5),
    }
}
