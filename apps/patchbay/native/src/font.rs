//! Fixed, build-validated GNU Unifont subset and allocation-free glyph drawing.

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::{DrawTarget, Pixel, Point};

pub const GLYPH_HEIGHT: usize = 16;
const REPLACEMENT_CODEPOINT: u32 = 0xFFFD;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextMetrics {
    pub advance: usize,
    pub missing_glyphs: usize,
}

#[derive(Clone, Copy)]
struct GlyphRecord {
    codepoint: u32,
    width: u8,
    bitmap: [u8; 32],
}

include!(concat!(env!("OUT_DIR"), "/unifont_subset.rs"));

pub struct BitmapFont;

impl BitmapFont {
    #[cfg(test)]
    pub fn glyph_count() -> usize {
        GLYPHS.len()
    }

    pub fn draw_character<T>(
        target: &mut T,
        origin: Point,
        character: char,
        color: Rgb888,
    ) -> Result<(usize, bool), T::Error>
    where
        T: DrawTarget<Color = Rgb888>,
    {
        let (glyph, missing) = Self::lookup(character);
        target.draw_iter(GlyphPixels::new(glyph, origin, color))?;
        Ok((usize::from(glyph.width), missing))
    }

    pub fn draw_text<T>(
        target: &mut T,
        origin: Point,
        text: &str,
        color: Rgb888,
    ) -> Result<TextMetrics, T::Error>
    where
        T: DrawTarget<Color = Rgb888>,
    {
        let mut advance = 0usize;
        let mut missing_glyphs = 0usize;
        for character in text.chars() {
            let x = origin
                .x
                .saturating_add(advance.min(i32::MAX as usize) as i32);
            let (width, missing) =
                Self::draw_character(target, Point::new(x, origin.y), character, color)?;
            advance = advance.saturating_add(width);
            missing_glyphs = missing_glyphs.saturating_add(usize::from(missing));
        }
        Ok(TextMetrics {
            advance,
            missing_glyphs,
        })
    }

    fn lookup(character: char) -> (&'static GlyphRecord, bool) {
        let codepoint = character as u32;
        match GLYPHS.binary_search_by_key(&codepoint, |glyph| glyph.codepoint) {
            Ok(index) => (&GLYPHS[index], false),
            Err(_) => {
                let index = GLYPHS
                    .binary_search_by_key(&REPLACEMENT_CODEPOINT, |glyph| glyph.codepoint)
                    .expect("build script requires replacement glyph");
                (&GLYPHS[index], true)
            }
        }
    }
}

struct GlyphPixels<'a> {
    glyph: &'a GlyphRecord,
    origin: Point,
    color: Rgb888,
    offset: usize,
}

impl<'a> GlyphPixels<'a> {
    fn new(glyph: &'a GlyphRecord, origin: Point, color: Rgb888) -> Self {
        Self {
            glyph,
            origin,
            color,
            offset: 0,
        }
    }
}

impl Iterator for GlyphPixels<'_> {
    type Item = Pixel<Rgb888>;

    fn next(&mut self) -> Option<Self::Item> {
        let width = usize::from(self.glyph.width);
        while self.offset < width * GLYPH_HEIGHT {
            let offset = self.offset;
            self.offset += 1;
            let row = offset / width;
            let column = offset % width;
            let byte = self.glyph.bitmap[row * (width / 8) + column / 8];
            let mask = 0x80 >> (column % 8);
            if byte & mask != 0 {
                return Some(Pixel(
                    Point::new(
                        self.origin.x.saturating_add(column as i32),
                        self.origin.y.saturating_add(row as i32),
                    ),
                    self.color,
                ));
            }
        }
        None
    }
}
