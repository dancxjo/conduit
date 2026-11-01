//! Authored metrics, fixed resources and actual raster work, not CSS imitation.
use super::*;
use crate::display::typography::{TextLayout, selected_glyph, selected_metrics};
fn tokens(body: u16, small: u16, line: u16) -> LookVisualTokensInfo {
    let look = conduit_face::engineering_look().unwrap();
    let conduit_face::LookVisualFacetOption::Some(v) = look.visual() else {
        panic!()
    };
    let b = v.tokens();
    LookVisualTokensInfo::new(
        *b.background(),
        *b.bootstrap_surface(),
        *b.emphasis(),
        *b.failure(),
        *b.focus(),
        *b.focus_width_px(),
        line,
        *b.muted(),
        *b.radius_control_px(),
        *b.radius_panel_px(),
        *b.reading_paper(),
        *b.space_control_block_px(),
        *b.space_control_inline_px(),
        *b.space_panel_px(),
        *b.space_unit_px(),
        *b.structure_primary(),
        *b.structure_secondary(),
        *b.success(),
        *b.surface(),
        *b.text_primary(),
        *b.text_secondary(),
        body,
        small,
        *b.warning(),
        *b.workbench_canvas(),
    )
    .unwrap()
}
#[test]
fn authored_sizes_and_leading_are_exact_and_change_the_same_cursor() {
    let typography =
        Typography::with_look_metrics(VisualTypography::Engineering, &tokens(18, 16, 150), 200)
            .unwrap();
    let style = typography.style(TextRole::Body);
    let metrics = selected_metrics(style);
    assert_eq!((metrics.em_size, metrics.line_height), (18, 27));
    assert_eq!(
        selected_metrics(typography.style(TextRole::Label)).em_size,
        16
    );
    assert_eq!(
        selected_metrics(typography.style(TextRole::Heading)).em_size,
        23
    );
    assert_eq!(
        selected_metrics(typography.style(TextRole::Title)).em_size,
        27
    );
    let base = selected_glyph(Typography::EDITORIAL.style(TextRole::Body), 'W');
    let wanted = selected_glyph(style, 'W');
    assert_eq!(
        u32::from(wanted.advance()),
        (u32::from(base.advance()) * 18).div_ceil(16)
    );
    let layout = TextLayout::new_selected("W\nW", style, 200, 100).unwrap();
    assert_eq!(layout.finish_height(), 54);
    let glyphs: alloc::vec::Vec<_> = TextLayout::new_selected("W\nW", style, 200, 100)
        .unwrap()
        .collect();
    assert_eq!(glyphs[1].pen.1 - glyphs[0].pen.1, 27);
    let more =
        Typography::with_look_metrics(VisualTypography::Engineering, &tokens(18, 16, 180), 200)
            .unwrap();
    assert_eq!(
        selected_glyph(more.style(TextRole::Body), 'W').advance(),
        wanted.advance()
    );
    assert_eq!(
        TextLayout::new_selected("W\nW", more.style(TextRole::Body), 200, 100)
            .unwrap()
            .finish_height(),
        66
    );
}
#[test]
fn complete_admitted_scale_range_preserves_existing_glyph_work_bound() {
    let typography =
        Typography::with_look_metrics(VisualTypography::Engineering, &tokens(18, 16, 150), 200)
            .unwrap();
    for role in [
        TextRole::Label,
        TextRole::Body,
        TextRole::Heading,
        TextRole::Title,
        TextRole::Code,
    ] {
        for percent in [100, 125, 200] {
            for item in TextLayout::new_selected(
                "AéJp\u{301}\u{10ffff}",
                typography.style(role),
                600,
                percent,
            )
            .unwrap()
            {
                let (width, height) = item.glyph.extent();
                assert!(width <= 128 && height <= 128);
                assert_eq!(item.glyph.coverage(width, height), 0);
            }
        }
    }
    assert!(matches!(
        Typography::with_look_metrics(VisualTypography::Engineering, &tokens(64, 14, 250), 200),
        Err(LookTypographyRefusal::GlyphBudget { .. })
    ));
    assert!(matches!(
        Typography::with_look_metrics(VisualTypography::Engineering, &tokens(18, 16, 100), 200),
        Err(LookTypographyRefusal::LineBox { .. })
    ));
}
#[test]
fn narrower_admitted_scale_cannot_be_bypassed_at_measurement_or_paint() {
    let typography =
        Typography::with_look_metrics(VisualTypography::Engineering, &tokens(24, 16, 150), 100)
            .unwrap();
    let style = typography.style(TextRole::Body);
    assert!(TextLayout::new_selected("room", style, 600, 100).is_ok());
    assert!(TextLayout::new_selected("room", style, 600, 125).is_err());
    assert!(crate::display::typography::intrinsic_text_metrics("room", style, 125).is_err());
    assert!(crate::display::typography::resolve_em_sixteenths(style, 125, 16).is_err());
    for scale in [0, 99, 201, u16::MAX] {
        assert_eq!(
            Typography::with_look_metrics(
                VisualTypography::Engineering,
                &tokens(18, 16, 150),
                scale
            ),
            Err(LookTypographyRefusal::MetricBounds)
        );
    }
}

struct Surface {
    pixels: alloc::vec::Vec<u32>,
}
impl crate::display::PixelTarget for Surface {
    fn format(&self) -> crate::display::DisplayFormat {
        crate::display::DisplayFormat {
            width: 256,
            height: 192,
            pitch: 1024,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(
        &mut self,
        x: u32,
        y: u32,
        pixel: u32,
    ) -> Result<(), crate::display::DisplayError> {
        self.pixels[y as usize * 256 + x as usize] = pixel;
        Ok(())
    }
}
impl crate::display::RetainedPixelTarget for Surface {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, crate::display::DisplayError> {
        Ok(self.pixels[y as usize * 256 + x as usize])
    }
}
#[test]
fn actual_raster_ink_fits_measured_authored_line_boxes_and_scale_refuses_before_writes() {
    use crate::display::typography::render_text_selected;
    use conduit_face::LayoutRect;
    let typography =
        Typography::with_look_metrics(VisualTypography::Engineering, &tokens(18, 16, 150), 200)
            .unwrap();
    for role in [
        TextRole::Label,
        TextRole::Body,
        TextRole::Heading,
        TextRole::Title,
        TextRole::Code,
    ] {
        for percent in [100, 125, 200] {
            let value = "AéJp\u{301}\nAéJp";
            let style = typography.style(role);
            let height = TextLayout::new_selected(value, style, 256, percent)
                .unwrap()
                .finish_height();
            for item in TextLayout::new_selected(value, style, 256, percent).unwrap() {
                let (x, y) = item.glyph.bearing();
                let (w, h) = item.glyph.extent();
                assert!(item.pen.0 + i32::from(x) >= 0);
                assert!(item.pen.1 + i32::from(y) >= 0);
                assert!(item.pen.0 + i32::from(x) + i32::from(w) <= 256);
                assert!(item.pen.1 + i32::from(y) + i32::from(h) <= height as i32);
            }
            let mut surface = Surface {
                pixels: alloc::vec![0x112233;256*192],
            };
            let bounds = LayoutRect {
                x: 0,
                y: 0,
                width: 256,
                height: height as u16,
            };
            let receipt = render_text_selected(
                &mut surface,
                value,
                style,
                bounds,
                bounds,
                0xffffff,
                percent,
            )
            .unwrap();
            assert!(receipt.pixels_written > 0);
            assert!(surface.pixels.iter().any(|pixel| *pixel != 0x112233));
        }
    }
    let typography =
        Typography::with_look_metrics(VisualTypography::Engineering, &tokens(24, 16, 150), 100)
            .unwrap();
    let mut surface = Surface {
        pixels: alloc::vec![0x112233;256*192],
    };
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 256,
        height: 192,
    };
    assert!(
        render_text_selected(
            &mut surface,
            "room",
            typography.style(TextRole::Body),
            bounds,
            bounds,
            0xffffff,
            125
        )
        .is_err()
    );
    assert!(surface.pixels.iter().all(|pixel| *pixel == 0x112233));
}
