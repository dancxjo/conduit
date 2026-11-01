//! Pixel proof of the production aliased and retained rectangle routes.
use super::*;
use crate::display::{DisplayFormat, RetainedPixelTarget};
use conduit_face::{GraphicsCommand, GraphicsPaintRole};

struct Target(alloc::vec::Vec<u32>);
impl Target {
    fn new() -> Self {
        Self(alloc::vec![0; 24 * 24])
    }
}
impl PixelTarget for Target {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 24,
            height: 24,
            pitch: 96,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        assert!(x < 24 && y < 24);
        self.0[(y * 24 + x) as usize] = pixel;
        Ok(())
    }
}
impl RetainedPixelTarget for Target {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        assert!(x < 24 && y < 24);
        Ok(self.0[(y * 24 + x) as usize])
    }
}
fn rect(style: GraphicsShapeStyle, radius: u8, stroke: u8) -> GraphicsCommand {
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 24,
        height: 24,
    };
    GraphicsCommand::rect(bounds, bounds, GraphicsPaintRole::Border, style)
        .unwrap()
        .with_rect_metrics(radius, stroke)
        .unwrap()
}
fn pixels(command: &GraphicsCommand, smooth: bool, clip: LayoutRect) -> Target {
    let mut target = Target::new();
    let mut receipt = DisplayReceipt::default();
    if smooth {
        render_smooth(&mut target, command, clip, 0xffffff, &mut receipt).unwrap();
    } else {
        render(&mut target, command, clip, 0xffffff, &mut receipt).unwrap();
    }
    assert_eq!(
        receipt.pixels_written as usize,
        target.0.iter().filter(|p| **p != 0).count()
    );
    target
}
#[test]
fn square_panels_and_explicit_strokes_have_exact_pixels_in_both_routes() {
    for smooth in [false, true] {
        let command = rect(GraphicsShapeStyle::RoundedFill, 0, 1);
        assert!(
            pixels(&command, smooth, command.bounds)
                .0
                .iter()
                .all(|p| *p == 0xffffff)
        );
        for stroke in 1..=8 {
            let command = rect(GraphicsShapeStyle::RoundedStroke, 0, stroke);
            let target = pixels(&command, smooth, command.bounds);
            let expected = 24 * 24 - (24 - usize::from(stroke) * 2).pow(2);
            assert_eq!(target.0.iter().filter(|p| **p != 0).count(), expected);
        }
    }
}
#[test]
fn every_radius_is_symmetric_and_clipped_pixels_equal_the_original_geometry() {
    let clip = LayoutRect {
        x: 1,
        y: 3,
        width: 12,
        height: 9,
    };
    for smooth in [false, true] {
        for radius in 0..=32 {
            for style in [
                GraphicsShapeStyle::RoundedFill,
                GraphicsShapeStyle::RoundedStroke,
            ] {
                let command = rect(style, radius, 3);
                let full = pixels(&command, smooth, command.bounds);
                let clipped = pixels(&command, smooth, clip);
                for y in 0..24usize {
                    for x in 0..24usize {
                        assert_eq!(full.0[y * 24 + x], full.0[(23 - y) * 24 + 23 - x]);
                        let visible = (1..13).contains(&x) && (3..12).contains(&y);
                        assert_eq!(
                            clipped.0[y * 24 + x],
                            if visible { full.0[y * 24 + x] } else { 0 }
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn changing_radius_changes_coverage_and_tiny_strokes_stay_visible() {
    for smooth in [false, true] {
        let square = rect(GraphicsShapeStyle::RoundedFill, 0, 1);
        let round = rect(GraphicsShapeStyle::RoundedFill, 6, 1);
        assert_ne!(
            pixels(&square, smooth, square.bounds).0,
            pixels(&round, smooth, round.bounds).0
        );
        for size in 1..=3 {
            let mut command = rect(GraphicsShapeStyle::RoundedStroke, 32, 8);
            command.bounds.width = size;
            command.bounds.height = size;
            let target = pixels(&command, smooth, command.bounds);
            assert!(target.0.iter().any(|p| *p != 0));
        }
    }
}
