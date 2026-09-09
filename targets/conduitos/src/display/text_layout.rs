//! Shared fixed-font wrapping for measurement and raster placement.
use super::font;
#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
use super::DisplayError;

pub(super) struct TextCursor {
    width: u32,
    x: u32,
    y: u32,
    previous: Option<(u32, u32)>,
}

impl TextCursor {
    pub(super) fn new(width: u16) -> Self {
        Self {
            width: u32::from(width),
            x: 0,
            y: 0,
            previous: None,
        }
    }

    pub(super) fn advance(&mut self, character: char) -> Option<(u32, u32)> {
        if character == '\n' {
            self.previous = None;
            self.x = 0;
            self.y = self.y.saturating_add(font::GLYPH_HEIGHT);
            return None;
        }
        // Only the two combining marks in the pinned naming coverage are
        // supported. An orphan mark keeps an ordinary cell; never reach back
        // across a newline. The payload itself is neither normalized nor edited.
        if let ('\u{0300}' | '\u{0301}', Some(position)) = (character, self.previous) {
            return Some(position);
        }
        let width = u32::from(font::glyph(character).0.width);
        if self.x + width > self.width {
            self.x = 0;
            self.y = self.y.saturating_add(font::GLYPH_HEIGHT);
        }
        let position = (self.x, self.y);
        self.x += width;
        self.previous = Some(position);
        Some(position)
    }
}

#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
pub(crate) fn text_height(value: &str, width: u16) -> Result<u16, DisplayError> {
    let mut cursor = TextCursor::new(width);
    for character in value.chars() {
        if character != '\n' && u16::from(font::glyph(character).0.width) > width {
            return Err(DisplayError::InvalidExtent);
        }
        cursor.advance(character);
    }
    u16::try_from(cursor.y + font::GLYPH_HEIGHT).map_err(|_| DisplayError::InvalidExtent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naming_accents_overlay_the_previous_cell_even_at_a_wrap() {
        let mut cursor = TextCursor::new(8);
        assert_eq!(cursor.advance('ẹ'), Some((0, 0)));
        assert_eq!(cursor.advance('\u{0301}'), Some((0, 0)));
        assert_eq!(cursor.advance('A'), Some((0, 16)));
        assert_eq!(cursor.advance('\u{0300}'), Some((0, 16)));
        assert_eq!(text_height("ẹ\u{0301}A\u{0300}", 8), Ok(32));
    }

    #[test]
    fn orphan_accents_do_not_reach_across_newlines() {
        let mut cursor = TextCursor::new(16);
        assert_eq!(cursor.advance('A'), Some((0, 0)));
        assert_eq!(cursor.advance('\n'), None);
        assert_eq!(cursor.advance('\u{0301}'), Some((0, 16)));
        assert_eq!(cursor.advance('B'), Some((8, 16)));
        assert_eq!(text_height("\u{0301}B", 16), Ok(16));
    }

    #[test]
    fn measurement_uses_glyph_widths_newlines_and_fallback() {
        assert_eq!(text_height("ABCD", 16), Ok(32));
        assert_eq!(text_height("AB\nCD", 16), Ok(32));
        assert_eq!(text_height("中A", 16), Ok(32));
        assert_eq!(text_height("A\n", 16), Ok(32));
        assert_eq!(text_height("🦀", 16), text_height("�", 16));
        assert_eq!(text_height("中", 8), Err(DisplayError::InvalidExtent));
    }
}
