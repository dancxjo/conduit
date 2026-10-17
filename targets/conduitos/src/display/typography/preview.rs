//! Bounded label preparation using the same pinned glyphs as rasterization.
use super::{TextRole, glyph};
use crate::display::DisplayError;
use alloc::{format, string::String};
use conduit_face::MAX_GRAPHICS_TEXT_BYTES;

/// Fit one default-scale line, reserving a visible ellipsis when clipped.
/// Examines at most one graphical payload of UTF-8, including zero-advance marks.
/// This preserves scalars; it does not provide general text shaping.
pub fn single_line_preview(
    value: &str,
    role: TextRole,
    width: u16,
) -> Result<String, DisplayError> {
    let capacity = u32::from(width) * 64;
    let dot = glyph(role, '.');
    let advance = u32::from(dot.advance());
    let ink = (i32::from(dot.bearing().0) + i32::from(dot.extent().0)).max(0) as u32 * 64;
    let suffix = advance * 2 + advance.max(ink);
    if capacity < suffix {
        return Err(DisplayError::InvalidExtent);
    }
    let (mut used, mut right, mut end, mut prefix) = (0u32, 0u32, 0usize, 0usize);
    for (offset, character) in value.char_indices() {
        if matches!(character, '\n' | '\t') {
            break;
        }
        let next = offset + character.len_utf8();
        if next > MAX_GRAPHICS_TEXT_BYTES {
            break;
        }
        let raster = glyph(role, character);
        let ink = (i32::from(raster.bearing().0) + i32::from(raster.extent().0)).max(0) as u32 * 64;
        let next_right = right.max(used + ink);
        let next_used = used + u32::from(raster.advance());
        if next_right.max(next_used) > capacity {
            break;
        }
        (used, right, end) = (next_used, next_right, next);
        if right.max(used + suffix) <= capacity && next <= MAX_GRAPHICS_TEXT_BYTES - 3 {
            prefix = next;
        }
    }
    if end == value.len() {
        Ok(value.into())
    } else {
        Ok(format!("{}...", &value[..prefix]))
    }
}
