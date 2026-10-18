//! Actual CPU glyph rasterization into bounded pixels, not guest scanout proof.
use super::*;
use crate::display::{DisplayError, DisplayFormat, PixelTarget, RetainedPixelTarget};
use alloc::vec;
use conduit_face::{VisualExpression, VisualUserConstraints, admit_visual_expression};

pub(crate) struct Pixels {
    width: u32,
    height: u32,
    values: Vec<u32>,
}
const GUARD: u32 = 0x5a8c1397;
impl Pixels {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        Self {
            width: u32::from(width),
            height: u32::from(height),
            values: vec![GUARD; usize::from(width) * usize::from(height) + 2],
        }
    }
    fn offset(&self, x: u32, y: u32) -> Result<usize, DisplayError> {
        if x >= self.width || y >= self.height {
            return Err(DisplayError::InvalidExtent);
        }
        Ok((y * self.width + x) as usize + 1)
    }
}
impl PixelTarget for Pixels {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: self.width,
            height: self.height,
            pitch: self.width * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, value: u32) -> Result<(), DisplayError> {
        let offset = self.offset(x, y)?;
        self.values[offset] = value;
        Ok(())
    }
}
impl RetainedPixelTarget for Pixels {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        Ok(self.values[self.offset(x, y)?])
    }
}
#[test]
fn measured_frames_rasterize_at_the_same_scale_without_touching_guard_pixels() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../../proof/conformance/face-waist/domain-derived/prebirth-face.json"
    ))
    .unwrap();
    for (width, height) in [(1280, 800), (640, 480), (320, 240)] {
        for scale in [100, 200] {
            let expression = admit_visual_expression(
                Ok(VisualExpression::phosphor()),
                VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: scale,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            let mut scene = NativeFaceScene::prepare_visual_fallback(
                face.clone(),
                width,
                height,
                true,
                &expression,
            )
            .unwrap();
            for details in [false, true] {
                scene.show_details(details);
                let frame = scene.frame().unwrap();
                let mut pixels = Pixels::new(width, height);
                let receipt = crate::display::typography::render_scene_scaled(
                    &mut pixels,
                    &frame.scene,
                    |_, command| command.text_role().into(),
                    scale,
                )
                .unwrap();
                assert_eq!(receipt.commands as usize, frame.scene.commands().len());
                assert!(receipt.pixels_written > 0);
                assert_eq!(pixels.values[0], GUARD);
                assert_eq!(pixels.values[pixels.values.len() - 1], GUARD);
                assert!(
                    pixels.values[1..pixels.values.len() - 1]
                        .windows(2)
                        .any(|pair| pair[0] != pair[1])
                );
                assert_eq!(scene.face(), &face);
            }
        }
    }
}
