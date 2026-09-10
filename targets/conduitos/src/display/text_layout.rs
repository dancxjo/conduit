//! Shared fixed-font wrapping for measurement and raster placement.
use super::DisplayError;
use super::font;

pub(super) struct TextCursor {
    width: u32,
    x: u32,
    y: u32,
}

impl TextCursor {
    pub(super) fn new(width: u16) -> Self {
        Self {
            width: u32::from(width),
            x: 0,
            y: 0,
        }
    }

    pub(super) fn advance(&mut self, character: char) -> Option<(u32, u32)> {
        if character == '\n' {
            self.x = 0;
            self.y = self.y.saturating_add(font::GLYPH_HEIGHT);
            return None;
        }
        let width = u32::from(font::glyph(character).0.width);
        if Self::is_combining_mark(character) {
            return Some((self.x.saturating_sub(width), self.y));
        }
        if self.x + width > self.width {
            self.x = 0;
            self.y = self.y.saturating_add(font::GLYPH_HEIGHT);
        }
        let position = (self.x, self.y);
        self.x += width;
        Some(position)
    }

    const fn is_combining_mark(character: char) -> bool {
        matches!(
            character as u32,
            0x0300..=0x036F
                | 0x1AB0..=0x1AFF
                | 0x1DC0..=0x1DFF
                | 0x20D0..=0x20FF
                | 0xFE20..=0xFE2F
        )
    }
}

pub(crate) fn text_height(value: &str, width: u16) -> Result<u16, DisplayError> {
    let mut cursor = TextCursor::new(width);
    for character in value.chars() {
        if character != '\n'
            && !TextCursor::is_combining_mark(character)
            && u16::from(font::glyph(character).0.width) > width
        {
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
    fn measurement_uses_glyph_widths_newlines_and_fallback() {
        assert_eq!(text_height("ABCD", 16), Ok(32));
        assert_eq!(text_height("AB\nCD", 16), Ok(32));
        assert_eq!(text_height("中A", 16), Ok(32));
        assert_eq!(text_height("A\n", 16), Ok(32));
        assert_eq!(text_height("🦀", 16), text_height("�", 16));
        assert_eq!(text_height("中", 8), Err(DisplayError::InvalidExtent));
    }
}
