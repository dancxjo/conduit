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
fn routed_path_rasterizes_only_its_clipped_axis_aligned_segments() {
    use conduit_presentation::{GraphicsPath, GraphicsPoint};
    let path = GraphicsPath::new(&[
        GraphicsPoint { x: -4, y: 4 },
        GraphicsPoint { x: 10, y: 4 },
        GraphicsPoint { x: 10, y: 20 },
        GraphicsPoint { x: 24, y: 20 },
    ])
    .unwrap();
    let clip = LayoutRect {
        x: 2,
        y: 2,
        width: 18,
        height: 20,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(GraphicsCommand::path(path, clip, GraphicsPaintRole::Accent).unwrap())
        .unwrap();
    let mut bytes = alloc::vec![0; 128 * 24];
    let receipt = render_scene(
        &mut Buffer {
            format: format(),
            bytes: &mut bytes,
            lost: false,
        },
        &scene,
    )
    .unwrap();
    assert_eq!(receipt.commands, 1);
    assert!(receipt.pixels_written <= 40);
    let color = paint(format(), GraphicsPaintRole::Accent).to_le_bytes();
    for y in 0..24 {
        for x in 0..32 {
            let offset = y * 128 + x * 4;
            let on_path = (y == 4 && (2..=10).contains(&x))
                || (x == 10 && (4..=20).contains(&y))
                || (y == 20 && (10..20).contains(&x));
            assert_eq!(
                &bytes[offset..offset + 4],
                if on_path { &color } else { &[0; 4] }
            );
        }
    }
}

#[test]
fn clipping_crops_text_without_restarting_or_rewrapping_it() {
    fn render(bounds: LayoutRect, clip: LayoutRect) -> alloc::vec::Vec<u8> {
        let mut bytes = alloc::vec![0; 128 * 24];
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::text(bounds, clip, GraphicsPaintRole::Foreground, "AB\nCD")
                    .unwrap(),
            )
            .unwrap();
        render_scene(
            &mut Buffer {
                format: format(),
                bytes: &mut bytes,
                lost: false,
            },
            &scene,
        )
        .unwrap();
        bytes
    }
    let full = LayoutRect {
        x: 0,
        y: 0,
        width: 32,
        height: 24,
    };
    let reference = render(full, full);
    let clip = LayoutRect {
        x: 4,
        y: 6,
        width: 24,
        height: 18,
    };
    let cropped = render(full, clip);
    for y in 0..24 {
        for x in 0..32 {
            let offset = y * 128 + x * 4;
            let expected = if y >= 6 && (4..28).contains(&x) {
                &reference[offset..offset + 4]
            } else {
                &[0; 4]
            };
            assert_eq!(&cropped[offset..offset + 4], expected);
        }
    }
    let scrolled = render(
        LayoutRect {
            y: -8,
            height: 32,
            ..full
        },
        full,
    );
    for y in 0..16 {
        assert_eq!(
            &scrolled[y * 128..(y + 1) * 128],
            &reference[(y + 8) * 128..(y + 9) * 128]
        );
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
    let first_glyph_pixel = 6 * 128 + 4 * 4;
    assert_eq!(
        &bytes[first_glyph_pixel..first_glyph_pixel + 4],
        &format().pixel(225, 232, 240).to_le_bytes()
    );

    let mut lost = Buffer {
        format: format(),
        bytes: &mut bytes,
        lost: true,
    };
    assert_eq!(render_scene(&mut lost, &scene), Err(DisplayError::Lost));
}

#[test]
fn pinned_unifont_subset_covers_ascii_and_multilingual_text() {
    assert_eq!(font::glyph_count(), 971);
    for character in ' '..='~' {
        let (glyph, missing) = font::glyph(character);
        assert!(!missing, "missing printable ASCII glyph {character:?}");
        if character != ' ' {
            assert!(glyph.bitmap.iter().any(|byte| *byte != 0));
        }
    }
    assert_eq!(
        &font::glyph('>').0.bitmap[..16],
        &[0, 0, 0, 0, 0, 0x40, 0x20, 0x10, 0x08, 0x04, 0x08, 0x10, 0x20, 0x40, 0, 0]
    );
    for character in ['é', 'Ω', 'Ж', '—', '→', '─', '■'] {
        assert!(
            !font::glyph(character).1,
            "missing subset glyph {character}"
        );
    }
    assert_eq!(font::glyph('中').0.width, 16);
    assert!(font::glyph('🦀').1);
}

#[test]
fn text_wraps_and_fallback_is_bounded_and_deterministic() {
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 16,
        height: 32,
    };
    let render = |value: &str| {
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, value)
                    .unwrap(),
            )
            .unwrap();
        let mut bytes = [0_u8; 32 * 24 * 4];
        let receipt = {
            let mut target = Buffer {
                format: format(),
                bytes: &mut bytes,
                lost: false,
            };
            render_scene(&mut target, &scene).unwrap()
        };
        (bytes, receipt)
    };

    let (wrapped, wrapped_receipt) = render("ABCD");
    let (explicit, explicit_receipt) = render("AB\nCD");
    assert_eq!(wrapped, explicit);
    assert_eq!(wrapped_receipt, explicit_receipt);

    let (fallback, fallback_receipt) = render("🦀🦀🦀");
    let (replacement, replacement_receipt) = render("���");
    assert_eq!(fallback, replacement);
    assert_eq!(fallback_receipt, replacement_receipt);
    assert!(fallback_receipt.pixels_written > 0);

    let (accented, _) = render("Aẹ\u{0301}BC");
    let (accented_lines, _) = render("Aẹ\u{0301}\nBC");
    assert_eq!(accented, accented_lines, "accent does not steal a cell");
    let (plain, _) = render("AẹBC");
    assert_ne!(accented, plain, "the accent is actually rasterized");
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
