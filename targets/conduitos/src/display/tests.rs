use super::*;
use conduit_presentation::{GraphicsCommand, GraphicsShapeStyle};

struct Buffer<'a> {
    format: DisplayFormat,
    bytes: &'a mut [u8],
    lost: bool,
}

impl PixelTarget for Buffer<'_> {
    fn format(&self) -> DisplayFormat {
        self.format
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if self.lost {
            return Err(DisplayError::Lost);
        }
        let offset =
            usize::try_from(u64::from(y) * u64::from(self.format.pitch) + u64::from(x) * 4)
                .map_err(|_| DisplayError::BufferTooSmall)?;
        let output = self
            .bytes
            .get_mut(offset..offset + 4)
            .ok_or(DisplayError::BufferTooSmall)?;
        output.copy_from_slice(&pixel.to_le_bytes());
        Ok(())
    }
}

fn format() -> DisplayFormat {
    DisplayFormat {
        width: 32,
        height: 24,
        pitch: 128,
        bits_per_pixel: 32,
        red_shift: 16,
        green_shift: 8,
        blue_shift: 0,
    }
}

#[test]
fn bounded_scene_renders_and_loss_remains_distinct() {
    let bounds = LayoutRect {
        x: 2,
        y: 2,
        width: 20,
        height: 12,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                bounds,
                bounds,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Stroke,
            )
            .unwrap(),
        )
        .unwrap();
    scene
        .push(GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, "OK").unwrap())
        .unwrap();
    let mut bytes = [0_u8; 32 * 24 * 4];
    let mut target = Buffer {
        format: format(),
        bytes: &mut bytes,
        lost: false,
    };
    let receipt = render_scene(&mut target, &scene).unwrap();
    assert_eq!(receipt.commands, 2);
    assert!(receipt.pixels_written > 0);
    assert!(bytes.iter().any(|byte| *byte != 0));
    let first_glyph_pixel = 2 * 128 + 3 * 4;
    assert_eq!(
        &bytes[first_glyph_pixel..first_glyph_pixel + 4],
        &format().pixel(205, 235, 224).to_le_bytes()
    );

    let mut lost = Buffer {
        format: format(),
        bytes: &mut bytes,
        lost: true,
    };
    assert_eq!(render_scene(&mut lost, &scene), Err(DisplayError::Lost));
}

#[test]
fn unsupported_format_and_small_buffer_refuse() {
    let mut invalid = format();
    invalid.bits_per_pixel = 24;
    assert_eq!(invalid.validate(), Err(DisplayError::UnsupportedFormat));

    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                bounds,
                bounds,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .unwrap(),
        )
        .unwrap();
    let mut bytes = [0_u8; 4];
    let mut target = Buffer {
        format: format(),
        bytes: &mut bytes,
        lost: false,
    };
    assert_eq!(
        render_scene(&mut target, &scene),
        Err(DisplayError::BufferTooSmall)
    );
}

#[test]
fn portable_gray8_bitmap_reaches_exact_framebuffer_pixels() {
    let bitmap =
        conduit_presentation::Gray8Bitmap::new(2, 2, alloc::vec![0, 64, 128, 255]).unwrap();
    let mut exact = format();
    exact.width = 4;
    exact.height = 4;
    exact.pitch = 16;
    let mut bytes = [0_u8; 4 * 4 * 4];
    {
        let mut target = Buffer {
            format: exact,
            bytes: &mut bytes,
            lost: false,
        };
        assert_eq!(
            render_gray8_bitmap(&mut target, &bitmap),
            Ok(DisplayReceipt {
                commands: 1,
                pixels_written: 16,
            })
        );
        target.lost = true;
        assert_eq!(
            render_gray8_bitmap(&mut target, &bitmap),
            Err(DisplayError::Lost)
        );
    }
    let pixels = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| u32::from_le_bytes(*pixel))
        .collect::<alloc::vec::Vec<_>>();
    assert_eq!(
        pixels,
        alloc::vec![
            0x0000_0000,
            0x0000_0000,
            0x0040_4040,
            0x0040_4040,
            0x0000_0000,
            0x0000_0000,
            0x0040_4040,
            0x0040_4040,
            0x0080_8080,
            0x0080_8080,
            0x00ff_ffff,
            0x00ff_ffff,
            0x0080_8080,
            0x0080_8080,
            0x00ff_ffff,
            0x00ff_ffff,
        ]
    );
}
