//! Finite software rendering for the native Patchbay surface.

use font8x8::UnicodeFonts;

pub const BACKGROUND: u32 = 0x0015_1820;
const FOREGROUND: u32 = 0x00e7_eaf0;
const ACCENT: u32 = 0x006d_d7c7;
const LEFT_MARGIN: usize = 16;
const TOP_MARGIN: usize = 16;
const GLYPH_ADVANCE: usize = 8;
const LINE_ADVANCE: usize = 11;

pub fn draw_document(buffer: &mut [u32], width: usize, height: usize, lines: &[String]) {
    for (line_index, line) in lines.iter().enumerate() {
        let y = TOP_MARGIN + line_index * LINE_ADVANCE;
        if y + 8 >= height {
            break;
        }
        let color = if line.starts_with("HOSTS")
            || line.starts_with("LINKS")
            || line.starts_with("OBSERVATIONS")
            || line.starts_with("CHECKED")
            || line.starts_with("DIAGNOSTIC")
            || line.starts_with("DISTRIBUTED")
            || line.starts_with("FORM ")
            || line.starts_with("PLAN-")
        {
            ACCENT
        } else {
            FOREGROUND
        };
        for (character_index, character) in line.chars().enumerate() {
            let x = LEFT_MARGIN + character_index * GLYPH_ADVANCE;
            if x + 8 >= width {
                break;
            }
            draw_character(buffer, width, x, y, character, color);
        }
    }
}

fn draw_character(
    buffer: &mut [u32],
    width: usize,
    x: usize,
    y: usize,
    character: char,
    color: u32,
) {
    let glyph = font8x8::BASIC_FONTS
        .get(character)
        .or_else(|| font8x8::BASIC_FONTS.get('?'))
        .unwrap_or([0; 8]);
    for (row, bits) in glyph.iter().enumerate() {
        for column in 0..8 {
            if bits & (1 << column) != 0 {
                buffer[(y + row) * width + x + column] = color;
            }
        }
    }
}
