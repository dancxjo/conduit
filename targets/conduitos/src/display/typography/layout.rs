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
    cluster_remaining: usize,
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
        // Reject an indivisible ink span before paint; never silently clip or
        // detach a following zero-advance mark to satisfy a narrow rectangle.
        let mut remaining = value;
        while !remaining.is_empty() {
            let bytes = super::ink::cluster_bytes(remaining, role, scale);
            if &remaining[..bytes] != "\n"
                && super::ink::span(&remaining[..bytes], role, scale, 0).fresh_width()
                    > u32::from(width)
            {
                return Err(DisplayError::InvalidExtent);
            }
            remaining = &remaining[bytes..];
        }
        Ok(Self {
            remaining: value,
            role,
            scale,
            width: u32::from(width),
            x: 0,
            line: 0,
            word_start: true,
            cluster_remaining: 0,
        })
    }

    fn newline(&mut self) {
        self.x = 0;
        self.line += 1;
    }

    /// End insertion position after the same wrapping used for glyph paint.
    pub fn finish_caret(mut self) -> (u32, u32, u32) {
        for _ in self.by_ref() {}
        let height = self
            .scale
            .unsigned(u32::from(metrics(self.role).line_height));
        (self.x / 64, self.line * height, height)
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
            if character == '\n' {
                self.remaining = &self.remaining[1..];
                self.newline();
                self.word_start = true;
                self.cluster_remaining = 0;
                continue;
            }
            let whitespace = character == ' ' || character == '\t';
            if self.word_start && !whitespace {
                let bytes = self
                    .remaining
                    .chars()
                    .take_while(|c| !matches!(c, ' ' | '\t' | '\n'))
                    .map(char::len_utf8)
                    .sum::<usize>();
                let word = &self.remaining[..bytes];
                let fresh = super::ink::span(word, self.role, self.scale, 0);
                if self.x > 0
                    && fresh.fresh_width() <= self.width
                    && !super::ink::span(word, self.role, self.scale, self.x).fits(self.width)
                {
                    self.newline();
                }
            }
            if self.cluster_remaining == 0 {
                let bytes = super::ink::cluster_bytes(self.remaining, self.role, self.scale);
                let cluster = &self.remaining[..bytes];
                if self.x > 0
                    && !super::ink::span(cluster, self.role, self.scale, self.x).fits(self.width)
                {
                    self.newline();
                    if whitespace {
                        self.remaining = &self.remaining[character.len_utf8()..];
                        self.word_start = true;
                        continue;
                    }
                }
                let span = super::ink::span(cluster, self.role, self.scale, self.x);
                if span.left < 0 {
                    self.x += (-span.left) as u32 * 64;
                }
                self.cluster_remaining = span.bytes;
            }
            self.remaining = &self.remaining[character.len_utf8()..];
            self.cluster_remaining -= character.len_utf8();
            self.word_start = whitespace;
            let raster = scaled_glyph(
                self.role,
                if character == '\t' { ' ' } else { character },
                self.scale,
            );
            let advance = u32::from(raster.advance()) * if character == '\t' { 4 } else { 1 };
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
        let narrow = (cell * 2).div_ceil(64) as u16;
        assert_eq!(TextLayout::new("abcdef", role, narrow).unwrap().count(), 6);
        assert!(matches!(
            TextLayout::new("abcdef", role, 1),
            Err(DisplayError::InvalidExtent)
        ));
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
