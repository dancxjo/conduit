//! Finite software rendering for the native Patchbay surface.

use font8x8::UnicodeFonts;
use patchbay_model::{PatchbayTheme, PHOSPHOR_THEME};

pub const BACKGROUND: u32 = PHOSPHOR_THEME.background.packed_rgb();
const LEFT_MARGIN: usize = 16;
const TOP_MARGIN: usize = 16;
const GLYPH_ADVANCE: usize = 8;
const LINE_ADVANCE: usize = 11;

pub fn draw_document(buffer: &mut [u32], width: usize, height: usize, lines: &[String]) {
    draw_vertical_rule(
        buffer,
        width,
        height,
        8,
        8,
        height.saturating_sub(16),
        PHOSPHOR_THEME.structure_secondary.packed_rgb(),
    );
    for (line_index, line) in lines.iter().enumerate() {
        let y = TOP_MARGIN + line_index * LINE_ADVANCE;
        if y + 8 >= height {
            break;
        }
        let heading = is_heading(line);
        let color = line_color(&PHOSPHOR_THEME, line, heading);
        for (character_index, character) in line.chars().enumerate() {
            let x = LEFT_MARGIN + character_index * GLYPH_ADVANCE;
            if x + 8 >= width {
                break;
            }
            draw_character(buffer, width, x, y, character, color);
        }
        if heading {
            draw_horizontal_rule(
                buffer,
                width,
                height,
                LEFT_MARGIN,
                y.saturating_add(9),
                width.saturating_sub(LEFT_MARGIN.saturating_mul(2)).min(96),
                PHOSPHOR_THEME.structure_primary.packed_rgb(),
            );
        }
    }
}

fn is_heading(line: &str) -> bool {
    line.starts_with("HOSTS")
        || line.starts_with("LINKS")
        || line.starts_with("OBSERVATIONS")
        || line.starts_with("CHECKED")
        || line.starts_with("DIAGNOSTIC")
        || line.starts_with("DISTRIBUTED")
        || line.starts_with("FORM ")
        || line.starts_with("PLAN-")
        || line.starts_with("ROUTE RECOVERY")
        || line.starts_with("NEW-PLAN")
        || line.starts_with("SAME-PLAN")
        || line.starts_with("UNPLANNED ROUTE")
        || line.starts_with("LINEAR NARRATION")
        || line.starts_with("ROUTE DETAIL")
}

fn line_color(theme: &PatchbayTheme, line: &str, heading: bool) -> u32 {
    if line.starts_with("> ") {
        theme.focus.packed_rgb()
    } else if heading {
        theme.emphasis.packed_rgb()
    } else if line.starts_with("  ") {
        theme.text_secondary.packed_rgb()
    } else {
        theme.text_primary.packed_rgb()
    }
}

fn draw_vertical_rule(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    length: usize,
    color: u32,
) {
    for row in y..y.saturating_add(length).min(height) {
        if let Some(pixel) = row
            .checked_mul(width)
            .and_then(|offset| offset.checked_add(x))
            .and_then(|index| buffer.get_mut(index))
        {
            *pixel = color;
        }
    }
}

fn draw_horizontal_rule(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    length: usize,
    color: u32,
) {
    if y >= height {
        return;
    }
    for column in x..x.saturating_add(length).min(width) {
        if let Some(pixel) = y
            .checked_mul(width)
            .and_then(|offset| offset.checked_add(column))
            .and_then(|index| buffer.get_mut(index))
        {
            *pixel = color;
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
                if let Some(pixel) = (y + row)
                    .checked_mul(width)
                    .and_then(|offset| offset.checked_add(x + column))
                    .and_then(|index| buffer.get_mut(index))
                {
                    *pixel = color;
                }
            }
        }
    }
}
