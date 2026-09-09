//! Fixed-radius rectangle rasterization, clipped without inventing new borders.
use super::{DisplayError, DisplayReceipt, GraphicsShapeStyle, LayoutRect, PixelTarget, put};

fn contains(x: i32, y: i32, width: i32, height: i32, radius: i32) -> bool {
    if x < 0 || y < 0 || x >= width || y >= height {
        return false;
    }
    let radius = radius.min(width / 2).min(height / 2);
    let dx = (radius - 1 - x).max(x - (width - radius)).max(0);
    let dy = (radius - 1 - y).max(y - (height - radius)).max(0);
    dx * dx + dy * dy <= radius * radius
}

pub(super) fn render(
    target: &mut impl PixelTarget,
    bounds: LayoutRect,
    clip: LayoutRect,
    shape: GraphicsShapeStyle,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let radius = i32::from(super::RADIUS);
    let border = i32::from(super::BORDER);
    let width = i32::from(bounds.width);
    let height = i32::from(bounds.height);
    for y in i32::from(clip.y)..i32::from(clip.y) + i32::from(clip.height) {
        for x in i32::from(clip.x)..i32::from(clip.x) + i32::from(clip.width) {
            let local_x = x - i32::from(bounds.x);
            let local_y = y - i32::from(bounds.y);
            if !contains(local_x, local_y, width, height, radius) {
                continue;
            }
            if shape == GraphicsShapeStyle::RoundedStroke
                && contains(
                    local_x - border,
                    local_y - border,
                    width - 2 * border,
                    height - 2 * border,
                    (radius - border).max(0),
                )
            {
                continue;
            }
            put(target, x as u32, y as u32, color, receipt)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Target;
    impl PixelTarget for Target {
        fn format(&self) -> super::super::DisplayFormat {
            super::super::DisplayFormat {
                width: 24,
                height: 24,
                pitch: 96,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }
        fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
            panic!("clipping an interior must not invent a border")
        }
    }

    #[test]
    fn interior_clip_does_not_create_a_new_stroke_edge() {
        let bounds = LayoutRect { x: 0, y: 0, width: 24, height: 24 };
        let clip = LayoutRect { x: 8, y: 8, width: 4, height: 4 };
        let mut receipt = DisplayReceipt::default();
        render(
            &mut Target,
            bounds,
            clip,
            GraphicsShapeStyle::RoundedStroke,
            1,
            &mut receipt,
        )
        .unwrap();
        assert_eq!(receipt.pixels_written, 0);
    }

    #[test]
    fn corners_are_bounded_symmetric_and_tiny_shapes_remain_visible() {
        assert!(!contains(0, 0, 24, 24, 6));
        assert!(contains(12, 0, 24, 24, 6));
        assert!(contains(0, 0, 1, 1, 6));
        assert!(!contains(0, 0, 0, 0, 6));
        for y in 0..24 {
            for x in 0..24 {
                assert_eq!(
                    contains(x, y, 24, 24, 6),
                    contains(23 - x, 23 - y, 24, 24, 6)
                );
            }
        }
    }
}
