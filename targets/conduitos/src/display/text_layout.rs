//! Shared fixed-font wrapping for measurement and raster placement.
#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
use super::DisplayError;
use super::font;

pub(super) struct TextCursor<'a> {
    remaining: &'a str,
    word_start: bool,
    width: u32,
    x: u32,
    y: u32,
}

impl<'a> TextCursor<'a> {
    pub(super) fn new(value: &'a str, width: u16) -> Self {
        Self {
            remaining: value,
            word_start: true,
            width: u32::from(width),
            x: 0,
            y: 0,
        }
    }

    fn advance(&mut self, character: char) -> Option<(u32, u32)> {
        if character == '\n' {
            self.x = 0;
            self.y = self.y.saturating_add(font::GLYPH_HEIGHT);
            return None;
        }
        let width = u32::from(font::glyph(character).0.width);
        // A trailing separator has no ink. Do not let it create an otherwise
        // empty soft-wrapped row before the next word or explicit newline.
        if character == ' ' && self.x + width > self.width {
            return None;
        }
        if self.x + width > self.width {
            self.x = 0;
            self.y = self.y.saturating_add(font::GLYPH_HEIGHT);
        }
        let position = (self.x, self.y);
        self.x += width;
        Some(position)
    }
}

impl Iterator for TextCursor<'_> {
    type Item = (char, Option<(u32, u32)>);

    fn next(&mut self) -> Option<Self::Item> {
        let character = self.remaining.chars().next()?;
        let separator = matches!(character, ' ' | '\n');
        if self.word_start && !separator && self.x > 0 {
            // Scan each word at most once. No copied text, growing buffer, or
            // scan of the suffix on every glyph; long words retain hard wrap.
            let mut word_width = 0_u32;
            for next in self
                .remaining
                .chars()
                .take_while(|next| !matches!(next, ' ' | '\n'))
            {
                word_width += u32::from(font::glyph(next).0.width);
                if word_width > self.width {
                    break;
                }
            }
            if word_width <= self.width && self.x + word_width > self.width {
                self.x = 0;
                self.y = self.y.saturating_add(font::GLYPH_HEIGHT);
            }
        }
        self.word_start = separator;
        self.remaining = &self.remaining[character.len_utf8()..];
        Some((character, self.advance(character)))
    }
}

#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
pub(crate) fn text_height(value: &str, width: u16) -> Result<u16, DisplayError> {
    let mut cursor = TextCursor::new(value, width);
    for (character, _) in cursor.by_ref() {
        if character != '\n' && u16::from(font::glyph(character).0.width) > width {
            return Err(DisplayError::InvalidExtent);
        }
    }
    u16::try_from(cursor.y + font::GLYPH_HEIGHT).map_err(|_| DisplayError::InvalidExtent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_fit_intact_and_measurement_matches_their_positions() {
        let cells: alloc::vec::Vec<_> = TextCursor::new("aa bb cc", 32).collect();
        assert_eq!(cells[3], ('b', Some((0, 16))));
        assert_eq!(cells[6], ('c', Some((0, 32))));
        assert_eq!(text_height("aa bb cc", 32), Ok(48));
        assert_eq!(text_height("AB CD", 16), Ok(32));
        assert_eq!(text_height("AB \nCD", 16), Ok(32));
        assert_eq!(text_height("ABCDEF", 16), Ok(48));
    }

    #[test]
    fn newlines_indentation_and_utf8_payload_remain_exact() {
        let value = "  A\n B 中A 🦀";
        let cells: alloc::vec::Vec<_> = TextCursor::new(value, 64).collect();
        assert_eq!(
            cells
                .iter()
                .map(|(character, _)| *character)
                .collect::<alloc::string::String>(),
            value
        );
        assert_eq!(cells[2], ('A', Some((16, 0))));
        assert_eq!(cells[5], ('B', Some((8, 16))));
        assert_eq!(text_height("A 中A", 24), Ok(32));
        assert_eq!(text_height("A 🦀", 16), text_height("A �", 16));
        assert_eq!(text_height("", 16), Ok(16));
        assert_eq!(text_height("A", 0), Err(DisplayError::InvalidExtent));
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
