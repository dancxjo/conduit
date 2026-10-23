//! Finite scalar ink spans in the same fixed-point cursor used for painting.
//! Following zero-advance marks stay with their base; this is not shaping.
use super::{TextScale, TextStyle, scaled_glyph};

pub(super) struct Span {
    pub bytes: usize,
    pub left: i32,
    pub right: i32,
}
impl Span {
    pub fn fits(&self, width: u32) -> bool {
        self.left >= 0 && self.right <= width as i32
    }
    pub fn fresh_width(&self) -> u32 {
        (self.right - self.left.min(0)) as u32
    }
}

pub(super) fn cluster_bytes(value: &str, role: TextStyle, scale: TextScale) -> usize {
    let mut chars = value.chars();
    let Some(first) = chars.next() else { return 0 };
    let mut bytes = first.len_utf8();
    if matches!(first, ' ' | '\t' | '\n') {
        return bytes;
    }
    for character in chars {
        if matches!(character, ' ' | '\t' | '\n')
            || scaled_glyph(role, character, scale).advance() != 0
        {
            break;
        }
        bytes += character.len_utf8();
    }
    bytes
}

pub(super) fn span(value: &str, role: TextStyle, scale: TextScale, start: u32) -> Span {
    let mut cursor = start;
    let mut left = (start / 64) as i32;
    let mut right = start.div_ceil(64) as i32;
    for character in value.chars() {
        let glyph = scaled_glyph(role, if character == '\t' { ' ' } else { character }, scale);
        let (width, height) = glyph.extent();
        if width > 0 && height > 0 {
            let origin = (cursor / 64) as i32 + i32::from(glyph.bearing().0);
            left = left.min(origin);
            right = right.max(origin + i32::from(width));
        }
        cursor += u32::from(glyph.advance()) * if character == '\t' { 4 } else { 1 };
    }
    right = right.max(cursor.div_ceil(64) as i32);
    Span {
        bytes: value.len(),
        left,
        right,
    }
}
