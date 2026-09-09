//! Shared bounded chooser geometry for painting, scrolling and hit testing.
use conduit_presentation::LayoutRect;

pub(super) fn row(bounds: LayoutRect, index: usize, scroll: u16) -> LayoutRect {
    let height = bounds
        .height
        .saturating_sub(88)
        .saturating_div(3)
        .clamp(20, 48);
    LayoutRect {
        x: 12,
        y: (76 + index as i32 * i32::from(height + 4) - i32::from(scroll)) as i16,
        width: bounds.width.saturating_sub(24).max(1),
        height,
    }
}

pub(super) fn content_height(bounds: LayoutRect) -> u16 {
    let last = row(bounds, 2, 0);
    last.y as u16 + last.height + 4
}

pub(super) fn hit(bounds: LayoutRect, scroll: u16, x: u16, y: u16) -> Option<u8> {
    if y < 70 || y >= bounds.height {
        return None;
    }
    (0..3)
        .find(|index| {
            let row = row(bounds, *index, scroll);
            i32::from(x) >= i32::from(row.x)
                && i32::from(x) < i32::from(row.x) + i32::from(row.width)
                && i32::from(y) >= i32::from(row.y)
                && i32::from(y) < i32::from(row.y) + i32::from(row.height)
        })
        .map(|index| index as u8)
}
