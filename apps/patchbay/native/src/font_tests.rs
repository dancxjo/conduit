use crate::canvas::SoftwareCanvas;
use crate::font::{BitmapFont, GLYPH_HEIGHT};
use embedded_graphics::pixelcolor::{Rgb888, RgbColor};
use embedded_graphics::prelude::Point;

#[test]
fn bounded_subset_covers_the_acceptance_scripts_and_double_width() {
    assert_eq!(BitmapFont::glyph_count(), 929);
    for character in ['A', 'é', 'Ω', 'Ж', '─', '→', '■'] {
        let mut pixels = [0; 16 * GLYPH_HEIGHT];
        let mut canvas = SoftwareCanvas::new(&mut pixels, 16, GLYPH_HEIGHT);
        let (width, missing) =
            BitmapFont::draw_character(&mut canvas, Point::zero(), character, Rgb888::WHITE)
                .unwrap();
        assert_eq!(width, 8, "unexpected width for {character}");
        assert!(!missing, "missing {character}");
        assert!(pixels.iter().any(|pixel| *pixel != 0));
    }

    let mut pixels = [0; 16 * GLYPH_HEIGHT];
    let mut canvas = SoftwareCanvas::new(&mut pixels, 16, GLYPH_HEIGHT);
    let (width, missing) =
        BitmapFont::draw_character(&mut canvas, Point::zero(), '中', Rgb888::WHITE).unwrap();
    assert_eq!(width, 16);
    assert!(!missing);
}

#[test]
fn missing_glyph_is_explicit_and_uses_a_bounded_replacement() {
    let mut pixels = [0; 16 * GLYPH_HEIGHT];
    let mut canvas = SoftwareCanvas::new(&mut pixels, 16, GLYPH_HEIGHT);
    let metrics = BitmapFont::draw_text(&mut canvas, Point::zero(), "A🦀", Rgb888::WHITE).unwrap();
    assert_eq!(metrics.advance, 16);
    assert_eq!(metrics.missing_glyphs, 1);
    assert!(pixels.iter().any(|pixel| *pixel != 0));
}

#[test]
fn clipped_glyph_does_not_touch_guard_pixels() {
    let guard = 0x0012_3456;
    let mut storage = [guard; 4 * 4 + 2];
    {
        let mut canvas = SoftwareCanvas::new(&mut storage[1..17], 4, 4);
        BitmapFont::draw_character(&mut canvas, Point::new(-4, -12), '中', Rgb888::WHITE).unwrap();
    }
    assert_eq!(storage[0], guard);
    assert_eq!(storage[17], guard);
}
