//! Allocation-free layout over one admitted graphical command payload.

use super::{GlyphRaster, TextRole, TextScale, metrics, scaled_glyph};
use crate::display::DisplayError;
use conduit_face::MAX_GRAPHICS_TEXT_BYTES;

pub struct PositionedGlyph {
    pub glyph: GlyphRaster,
    /// Baseline-relative position in pixels from the text rectangle's top left.
    pub pen: (i32, i32),
}

/// Word wrapping with hard breaks for long tokens. Lookahead examines each
/// token once; emitted positions and measured height share this exact cursor.
pub struct TextLayout<'a> {
    remaining: &'a str,
    role: TextRole,
    scale: TextScale,
    width: u32,
    x: u32,
    line: u32,
    word_start: bool,
}

impl<'a> TextLayout<'a> {
    pub fn new(value: &'a str, role: TextRole, width: u16) -> Result<Self, DisplayError> {
        Self::new_scaled(value, role, width, 100)
    }
    pub fn new_scaled(
        value: &'a str,
        role: TextRole,
        width: u16,
        text_scale_percent: u16,
    ) -> Result<Self, DisplayError> {
        let scale = TextScale::new(text_scale_percent)?;
        if value.len() > MAX_GRAPHICS_TEXT_BYTES {
            return Err(DisplayError::BufferTooSmall);
        }
        if width == 0 {
            return Err(DisplayError::InvalidExtent);
        }
        Ok(Self {
            remaining: value,
            role,
            scale,
            width: u32::from(width) * 64,
            x: 0,
            line: 0,
            word_start: true,
        })
    }

    fn newline(&mut self) {
        self.x = 0;
        self.line += 1;
    }

    /// Height after consuming the whole iterator, including a trailing newline.
    pub fn finish_height(mut self) -> u32 {
        for _ in self.by_ref() {}
        (self.line + 1)
            * self
                .scale
                .unsigned(u32::from(metrics(self.role).line_height))
    }
}

impl Iterator for TextLayout<'_> {
    type Item = PositionedGlyph;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let character = self.remaining.chars().next()?;
            self.remaining = &self.remaining[character.len_utf8()..];
            if character == '\n' {
                self.newline();
                self.word_start = true;
                continue;
            }
            let whitespace = character == ' ' || character == '\t';
            let raster = scaled_glyph(
                self.role,
                if character == '\t' { ' ' } else { character },
                self.scale,
            );
            let advance = u32::from(raster.advance()) * if character == '\t' { 4 } else { 1 };
            if self.word_start && !whitespace {
                let word_width = advance
                    + self
                        .remaining
                        .chars()
                        .take_while(|character| !matches!(character, ' ' | '\t' | '\n'))
                        .map(|character| {
                            u32::from(scaled_glyph(self.role, character, self.scale).advance())
                        })
                        .sum::<u32>();
                if self.x > 0 && word_width <= self.width && self.x + word_width > self.width {
                    self.newline();
                }
            }
            self.word_start = whitespace;
            if self.x > 0 && self.x + advance > self.width {
                self.newline();
                if whitespace {
                    continue;
                }
            }
            let pen = (
                (self.x / 64) as i32,
                (self.line
                    * self
                        .scale
                        .unsigned(u32::from(metrics(self.role).line_height)))
                    as i32
                    + self.scale.unsigned(metrics(self.role).ascent as u32) as i32,
            );
            self.x += advance;
            return Some(PositionedGlyph { glyph: raster, pen });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::glyph;
    use super::*;

    #[test]
    fn admitted_scale_changes_the_same_glyphs_and_line_metrics() {
        for percent in [100, 125, 200] {
            let scale = TextScale::new(percent).unwrap();
            let role = TextRole::Body;
            let items: alloc::vec::Vec<_> = TextLayout::new_scaled("W\ni", role, 100, percent)
                .unwrap()
                .collect();
            assert_eq!(
                items[1].pen.1 - items[0].pen.1,
                scale.unsigned(u32::from(metrics(role).line_height)) as i32
            );
            assert_eq!(
                TextLayout::new_scaled("W\ni", role, 100, percent)
                    .unwrap()
                    .finish_height(),
                2 * scale.unsigned(u32::from(metrics(role).line_height))
            );
            let original = glyph(role, 'W');
            let (w, h) = original.extent();
            assert_eq!(
                items[0].glyph.extent(),
                (
                    scale.unsigned(u32::from(w)) as u8,
                    scale.unsigned(u32::from(h)) as u8
                )
            );
            assert_eq!(
                items[0].glyph.advance(),
                scale.unsigned(u32::from(original.advance())) as u16
            );
        }
        for percent in [0, 99, 201, u16::MAX] {
            assert!(TextLayout::new_scaled("W", TextRole::Body, 100, percent).is_err());
        }
    }

    #[test]
    fn explicit_lines_and_height_use_same_metrics() {
        let role = TextRole::Body;
        let line = u32::from(metrics(role).line_height);
        assert_eq!(
            TextLayout::new("a\n", role, 100).unwrap().finish_height(),
            2 * line
        );
        let positions: alloc::vec::Vec<_> = TextLayout::new("a\nb", role, 100)
            .unwrap()
            .map(|item| item.pen)
            .collect();
        assert_eq!(positions[1].1 - positions[0].1, line as i32);
        assert_eq!(positions[1].0, 0);
    }

    #[test]
    fn words_wrap_together_and_long_tokens_make_progress() {
        let role = TextRole::Code;
        let cell = u32::from(glyph(role, 'M').advance());
        let width = (cell * 5).div_ceil(64) as u16;
        let positions: alloc::vec::Vec<_> = TextLayout::new("ab cde", role, width)
            .unwrap()
            .map(|item| item.pen)
            .collect();
        assert_eq!(positions[3].0, 0);
        assert!(positions[3].1 > positions[0].1);
        assert_eq!(positions[3].1, positions[5].1);
        assert_eq!(TextLayout::new("abcdef", role, 1).unwrap().count(), 6);
    }

    #[test]
    fn byte_admission_and_unicode_iteration_are_exact() {
        assert!(matches!(
            TextLayout::new("a", TextRole::Body, 0),
            Err(DisplayError::InvalidExtent)
        ));
        let oversized = "a".repeat(MAX_GRAPHICS_TEXT_BYTES + 1);
        assert!(matches!(
            TextLayout::new(&oversized, TextRole::Body, 100),
            Err(DisplayError::BufferTooSmall)
        ));
        assert_eq!(
            TextLayout::new("Trần ẹ\u{300}", TextRole::Body, 100)
                .unwrap()
                .count(),
            7
        );
    }
}
