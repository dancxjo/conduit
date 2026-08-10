//! Finite software rendering for the native Patchbay surface.

use crate::canvas::{softbuffer_to_rgb888, SoftwareCanvas};
use crate::font::{BitmapFont, GLYPH_HEIGHT};
use embedded_graphics::prelude::Point;
use patchbay_model::{PatchbayTheme, PHOSPHOR_THEME};

pub const BACKGROUND: u32 = PHOSPHOR_THEME.background.packed_rgb();
const LEFT_MARGIN: usize = 16;
const TOP_MARGIN: usize = 16;
const LINE_ADVANCE: usize = 19;

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
        let y = TOP_MARGIN.saturating_add(line_index.saturating_mul(LINE_ADVANCE));
        if y.saturating_add(GLYPH_HEIGHT) > height {
            break;
        }
        let heading = is_heading(line);
        let color = line_color(&PHOSPHOR_THEME, line, heading);
        let mut canvas = SoftwareCanvas::new(buffer, width, height);
        let _ = BitmapFont::draw_text(
            &mut canvas,
            Point::new(LEFT_MARGIN as i32, y as i32),
            line,
            softbuffer_to_rgb888(color),
        )
        .expect("software canvas drawing is infallible");
        if heading {
            draw_horizontal_rule(
                buffer,
                width,
                height,
                LEFT_MARGIN,
                y.saturating_add(GLYPH_HEIGHT + 1),
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

fn drawable_rows(buffer: &[u32], width: usize, height: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let complete = buffer.len() / width;
    let partial = usize::from(!buffer.len().is_multiple_of(width));
    complete.saturating_add(partial).min(height)
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
    let rows = drawable_rows(buffer, width, height);
    for row in y..y.saturating_add(length).min(rows) {
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
    if y >= drawable_rows(buffer, width, height) {
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
