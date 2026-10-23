//! Exact immutable ensemble bounds; scalar coverage, not general shaping.
use super::*;

#[test]
fn every_selected_fallback_and_rescue_ink_fits_scaled_line_metrics() {
    for role in [
        TextRole::Label,
        TextRole::Body,
        TextRole::Heading,
        TextRole::Title,
        TextRole::Code,
    ] {
        let profile = metrics(role);
        for percent in 100..=200 {
            let scale = TextScale::new(percent).unwrap();
            let baseline = scale.unsigned(profile.ascent as u32) as i32;
            let height = scale.unsigned(u32::from(profile.line_height)) as i32;
            for character in GLYPHS
                .iter()
                .filter(|g| g.profile == role as u8)
                .map(|g| char::from_u32(g.codepoint).unwrap())
                .chain(['中', '\u{10ffff}'])
            {
                let glyph = scaled_glyph(role, character, scale);
                let top = baseline + i32::from(glyph.bearing().1);
                let bottom = top + i32::from(glyph.extent().1);
                assert!(
                    top >= 0 && bottom <= height,
                    "{role:?}/{percent}%/{character:?}: ink{top}..{bottom}, line{height}"
                );
            }
        }
    }
}

#[test]
fn serif_title_keeps_proportional_advance_and_zero_advance_marks() {
    assert_ne!(
        glyph(TextRole::Title, 'i').advance(),
        glyph(TextRole::Title, 'W').advance()
    );
    assert_eq!(glyph(TextRole::Title, '\u{301}').advance(), 0);
    assert!(!glyph(TextRole::Title, 'É').unsupported);
    assert!(
        !glyph(TextRole::Title, '\u{3e2}').unsupported,
        "pinnedSans fallback remains admitted"
    );
    assert!(glyph(TextRole::Title, '\u{10ffff}').unsupported);
}

#[test]
fn serif_ink_wrap_and_caret_use_actual_scaled_edges() {
    for percent in [100, 125, 200] {
        let scale = TextScale::new(percent).unwrap();
        for value in ["JjW", "WW e\u{301}", "J W J", "\u{301}J", "Éé\u{301} J"] {
            for width in [64, 100] {
                let layout =
                    TextLayout::new_scaled(value, TextRole::Title, width, percent).unwrap();
                let measured = TextLayout::new_scaled(value, TextRole::Title, width, percent)
                    .unwrap()
                    .finish_height();
                let positioned = layout.collect::<alloc::vec::Vec<_>>();
                for (index, p) in positioned.iter().enumerate() {
                    let (w, h) = p.glyph.extent();
                    if w > 0 && h > 0 {
                        let (x, y) = p.glyph.bearing();
                        assert!(p.pen.0 + i32::from(x) >= 0);
                        assert!(p.pen.0 + i32::from(x) + i32::from(w) <= i32::from(width));
                        assert!(p.pen.1 + i32::from(y) >= 0);
                        assert!(p.pen.1 + i32::from(y) + i32::from(h) <= measured as i32);
                    }
                    if p.glyph.advance() == 0 && index > 0 {
                        assert_eq!(
                            p.pen.1,
                            positioned[index - 1].pen.1,
                            "zero-advance mark detached from its base"
                        );
                    }
                }
            }
        }
        let first = TextLayout::new_scaled("J", TextRole::Title, 100, percent)
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(
            first.pen.0,
            -i32::from(scaled_glyph(TextRole::Title, 'J', scale).bearing().0)
        );
        let caret = TextLayout::new_scaled("J", TextRole::Title, 100, percent)
            .unwrap()
            .finish_caret();
        assert_eq!(
            caret.0,
            first.pen.0 as u32 + u32::from(first.glyph.advance()) / 64
        );
        assert!(matches!(
            TextLayout::new_scaled("W\u{301}", TextRole::Title, 1, percent),
            Err(crate::display::DisplayError::InvalidExtent)
        ));
    }
}

struct Surface([u32; 128 * 128]);
impl crate::display::PixelTarget for Surface {
    fn format(&self) -> crate::display::DisplayFormat {
        crate::display::DisplayFormat {
            width: 128,
            height: 128,
            pitch: 512,
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
        self.0[y as usize * 128 + x as usize] = pixel;
        Ok(())
    }
}
impl crate::display::RetainedPixelTarget for Surface {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, crate::display::DisplayError> {
        Ok(self.0[y as usize * 128 + x as usize])
    }
}

#[test]
fn serif_and_mono_paint_every_measured_ink_sample_with_the_same_cursor() {
    for role in [TextRole::Title, TextRole::Code] {
        for percent in [100, 125, 200] {
            for value in ["JjW", "e\u{301}J", "中J"] {
                let bounds = conduit_face::LayoutRect {
                    x: 3,
                    y: 3,
                    width: 100,
                    height: 120,
                };
                let mut whole = Surface([0x204060; 128 * 128]);
                let mut individual = Surface(whole.0);
                render_text_scaled(&mut whole, value, role, bounds, bounds, 0xffffff, percent)
                    .unwrap();
                for p in TextLayout::new_scaled(value, role, bounds.width, percent).unwrap() {
                    render_glyph(
                        &mut individual,
                        &p.glyph,
                        (p.pen.0 + 3, p.pen.1 + 3),
                        bounds,
                        0xffffff,
                    )
                    .unwrap();
                }
                assert_eq!(whole.0, individual.0);
                assert!(whole.0.iter().any(|p| *p != 0x204060));
            }
        }
    }
}

#[test]
fn fixed_ensemble_resource_bytes_remain_under_existing_admission() {
    extern crate std;
    assert!(COVERAGE.len() <= 4 * 1_048_576);
    assert!(GLYPHS.len() <= 6 * 1_024);
    assert_eq!(PROFILES.len(), 6);
    assert!(GLYPHS.iter().any(|glyph| glyph.profile == 5));
    std::println!(
        "NATIVE_TYPOGRAPHY_RESOURCES coverage={} glyph_records={} glyph_record_bytes={} profile_bytes={}",
        COVERAGE.len(),
        GLYPHS.len(),
        core::mem::size_of_val(GLYPHS),
        core::mem::size_of_val(PROFILES)
    );
}

#[test]
fn maximum_command_unicode_clusters_make_bounded_narrow_progress() {
    let value = "e\u{301}".repeat(64);
    assert_eq!(value.len(), conduit_face::MAX_GRAPHICS_TEXT_BYTES);
    for percent in [100, 125, 200] {
        let glyphs = TextLayout::new_scaled(&value, TextRole::Title, 64, percent)
            .unwrap()
            .collect::<alloc::vec::Vec<_>>();
        assert_eq!(glyphs.len(), 128);
        for pair in glyphs.as_chunks::<2>().0 {
            assert_eq!(pair[1].glyph.advance(), 0);
            assert_eq!(pair[0].pen.1, pair[1].pen.1);
        }
        assert!(glyphs.last().unwrap().pen.1 > glyphs[0].pen.1);
    }
}
