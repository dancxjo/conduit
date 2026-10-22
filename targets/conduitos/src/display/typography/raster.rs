//! Coverage compositing into an already admitted retained surface.

use super::GlyphRaster;
use crate::display::{DisplayError, DisplayReceipt, RetainedPixelTarget};
use conduit_face::LayoutRect;

/// Lay out and draw one bounded command using the same cursor as measurement.
pub fn render_text(
    target: &mut impl RetainedPixelTarget,
    value: &str,
    role: super::TextRole,
    bounds: LayoutRect,
    clip: LayoutRect,
    color: u32,
) -> Result<DisplayReceipt, DisplayError> {
    render_text_scaled(target, value, role, bounds, clip, color, 100)
}

/// Fixed four-tap coverage filtering, bounded to twice each glyph extent.
pub fn render_text_scaled(
    target: &mut impl RetainedPixelTarget,
    value: &str,
    role: super::TextRole,
    bounds: LayoutRect,
    clip: LayoutRect,
    color: u32,
    text_scale_percent: u16,
) -> Result<DisplayReceipt, DisplayError> {
    render_text_selected(
        target,
        value,
        role.into(),
        bounds,
        clip,
        color,
        text_scale_percent,
    )
}

pub fn render_text_selected(
    target: &mut impl RetainedPixelTarget,
    value: &str,
    role: super::TextStyle,
    bounds: LayoutRect,
    clip: LayoutRect,
    color: u32,
    text_scale_percent: u16,
) -> Result<DisplayReceipt, DisplayError> {
    let layout = super::TextLayout::new_selected(value, role, bounds.width, text_scale_percent)?;
    let format = target.format().validate()?;
    let mut receipt = DisplayReceipt::default();
    let Some(clip) = super::super::clipped(bounds, clip, format) else {
        return Ok(receipt);
    };
    for positioned in layout {
        let pen = (
            i32::from(bounds.x) + positioned.pen.0,
            i32::from(bounds.y) + positioned.pen.1,
        );
        let glyph_receipt = render_glyph(target, &positioned.glyph, pen, clip, color)?;
        receipt.pixels_written = receipt
            .pixels_written
            .checked_add(glyph_receipt.pixels_written)
            .ok_or(DisplayError::InvalidExtent)?;
    }
    receipt.commands = 1;
    Ok(receipt)
}

/// Draw one glyph at a baseline pen, clipped to the retained target and clip.
/// Work is at most 128 by 128 samples. Transparent samples do not read or write.
pub fn render_glyph(
    target: &mut impl RetainedPixelTarget,
    glyph: &GlyphRaster,
    pen: (i32, i32),
    clip: LayoutRect,
    color: u32,
) -> Result<DisplayReceipt, DisplayError> {
    let format = target.format().validate()?;
    let (bearing_x, bearing_y) = glyph.bearing();
    let origin_x = pen
        .0
        .checked_add(i32::from(bearing_x))
        .ok_or(DisplayError::InvalidExtent)?;
    let origin_y = pen
        .1
        .checked_add(i32::from(bearing_y))
        .ok_or(DisplayError::InvalidExtent)?;
    let (width, height) = glyph.extent();
    let mut receipt = DisplayReceipt::default();
    for row in 0..height {
        for column in 0..width {
            let x = i64::from(origin_x) + i64::from(column);
            let y = i64::from(origin_y) + i64::from(row);
            if x < 0
                || y < 0
                || x >= i64::from(format.width)
                || y >= i64::from(format.height)
                || x < i64::from(clip.x)
                || y < i64::from(clip.y)
                || x >= i64::from(clip.x) + i64::from(clip.width)
                || y >= i64::from(clip.y) + i64::from(clip.height)
            {
                continue;
            }
            let coverage = glyph.coverage(column, row);
            if coverage == 0 {
                continue;
            }
            let x = x as u32;
            let y = y as u32;
            let background = target.read_pixel(x, y)?;
            let mut pixel = background;
            for shift in [format.red_shift, format.green_shift, format.blue_shift] {
                let foreground = (color >> shift) & 255;
                let previous = (background >> shift) & 255;
                let alpha = u32::from(coverage);
                let channel = (foreground * alpha + previous * (255 - alpha) + 127) / 255;
                pixel = (pixel & !(255 << shift)) | (channel << shift);
            }
            target.write_pixel(x, y, pixel)?;
            receipt.pixels_written += 1;
        }
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::super::{TextRole, glyph};
    use super::*;
    use crate::display::{DisplayFormat, PixelTarget};

    struct Surface {
        pixels: [u32; 32 * 32],
    }

    #[test]
    fn scaled_text_raster_uses_the_measured_cursor_and_preserves_default() {
        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width: 32,
            height: 32,
        };
        for percent in [100, 125, 200] {
            let mut rendered = Surface {
                pixels: [0x204060; 32 * 32],
            };
            let mut measured = Surface {
                pixels: rendered.pixels,
            };
            let receipt = render_text_scaled(
                &mut rendered,
                "Wi\ni",
                TextRole::Body,
                bounds,
                bounds,
                0xffffff,
                percent,
            )
            .unwrap();
            let mut count = 0;
            for item in
                super::super::TextLayout::new_scaled("Wi\ni", TextRole::Body, 32, percent).unwrap()
            {
                count += render_glyph(&mut measured, &item.glyph, item.pen, bounds, 0xffffff)
                    .unwrap()
                    .pixels_written;
            }
            assert_eq!(rendered.pixels, measured.pixels);
            assert_eq!(receipt.pixels_written, count);
            assert!(count > 0);
            if percent == 100 {
                measured.pixels.fill(0x204060);
                render_text(
                    &mut measured,
                    "Wi\ni",
                    TextRole::Body,
                    bounds,
                    bounds,
                    0xffffff,
                )
                .unwrap();
                assert_eq!(rendered.pixels, measured.pixels);
            }
        }
        let mut surface = Surface {
            pixels: [0x204060; 32 * 32],
        };
        assert!(
            render_text_scaled(
                &mut surface,
                "W",
                TextRole::Body,
                bounds,
                bounds,
                0xffffff,
                201
            )
            .is_err()
        );
        assert!(surface.pixels.iter().all(|pixel| *pixel == 0x204060));
    }

    #[test]
    fn oversized_text_refuses_before_any_surface_write_even_when_clipped() {
        let oversized = "x".repeat(conduit_face::MAX_GRAPHICS_TEXT_BYTES + 1);
        let mut surface = Surface {
            pixels: [0x204060; 32 * 32],
        };
        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width: 32,
            height: 32,
        };
        for clip in [
            bounds,
            LayoutRect {
                x: 40,
                y: 40,
                width: 1,
                height: 1,
            },
        ] {
            let result = render_text(
                &mut surface,
                &oversized,
                TextRole::Body,
                bounds,
                clip,
                0xffffff,
            );
            assert!(matches!(result, Err(DisplayError::BufferTooSmall)));
            assert_eq!(surface.pixels, [0x204060; 32 * 32]);
        }
    }

    #[test]
    fn unrenderable_one_pixel_unicode_refuses_before_writing_ink() {
        let value = "é".repeat(conduit_face::MAX_GRAPHICS_TEXT_BYTES / 2);
        assert!(matches!(
            super::super::TextLayout::new(&value, TextRole::Body, 1),
            Err(DisplayError::InvalidExtent)
        ));
        let mut surface = Surface {
            pixels: [0x204060; 32 * 32],
        };
        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width: 1,
            height: 32,
        };
        let receipt = render_text(
            &mut surface,
            &value,
            TextRole::Body,
            bounds,
            bounds,
            0xffffff,
        );
        assert!(matches!(receipt, Err(DisplayError::InvalidExtent)));
        assert_eq!(surface.pixels, [0x204060; 32 * 32]);
    }

    #[test]
    fn text_layout_raster_is_repeatable_and_confined_to_its_rectangle() {
        let bounds = LayoutRect {
            x: 3,
            y: 2,
            width: 20,
            height: 26,
        };
        let mut surface = Surface {
            pixels: [0x204060; 32 * 32],
        };
        let receipt = render_text(
            &mut surface,
            "Wi\nề",
            TextRole::Body,
            bounds,
            bounds,
            0xffffff,
        )
        .unwrap();
        assert_eq!(receipt.commands, 1);
        assert!(receipt.pixels_written > 0);
        let expected = surface.pixels;
        for _ in 0..64 {
            surface.pixels.fill(0x204060);
            let repeated = render_text(
                &mut surface,
                "Wi\nề",
                TextRole::Body,
                bounds,
                bounds,
                0xffffff,
            )
            .unwrap();
            assert_eq!(repeated.pixels_written, receipt.pixels_written);
            assert_eq!(surface.pixels, expected);
        }
        for y in 0..32 {
            for x in 0..32 {
                if !(3..23).contains(&x) || !(2..28).contains(&y) {
                    assert_eq!(surface.pixels[y * 32 + x], 0x204060);
                }
            }
        }
    }
    impl PixelTarget for Surface {
        fn format(&self) -> DisplayFormat {
            DisplayFormat {
                width: 32,
                height: 32,
                pitch: 128,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }
        fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
            self.pixels[y as usize * 32 + x as usize] = pixel;
            Ok(())
        }
    }
    impl RetainedPixelTarget for Surface {
        fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
            Ok(self.pixels[y as usize * 32 + x as usize])
        }
    }

    #[test]
    fn grayscale_blends_retained_background_and_preserves_clip() {
        let mut surface = Surface {
            pixels: [0xa0204060; 32 * 32],
        };
        let clip = LayoutRect {
            x: 5,
            y: 5,
            width: 12,
            height: 12,
        };
        let receipt = render_glyph(
            &mut surface,
            &glyph(TextRole::Heading, 'W'),
            (2, 20),
            clip,
            0xffffff,
        )
        .unwrap();
        assert!(receipt.pixels_written > 0 && receipt.pixels_written <= 144);
        let mut smooth = false;
        for y in 0..32 {
            for x in 0..32 {
                let pixel = surface.pixels[y * 32 + x];
                assert_eq!(pixel >> 24, 0xa0);
                if !(5..17).contains(&x) || !(5..17).contains(&y) {
                    assert_eq!(pixel, 0xa0204060);
                }
                if pixel != 0xa0204060 && pixel != 0xa0ffffff {
                    smooth = true;
                }
            }
        }
        assert!(smooth);
    }
}
