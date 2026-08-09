//! Finite software rendering for the native Patchbay surface.

use crate::canvas::{softbuffer_to_rgb888, SoftwareCanvas};
use crate::font::{BitmapFont, GLYPH_HEIGHT};
use embedded_graphics::prelude::Point;

pub const BACKGROUND: u32 = 0x0015_1820;
const FOREGROUND: u32 = 0x00e7_eaf0;
const ACCENT: u32 = 0x006d_d7c7;
const LEFT_MARGIN: usize = 16;
const TOP_MARGIN: usize = 16;
const LINE_ADVANCE: usize = 19;

pub fn draw_document(buffer: &mut [u32], width: usize, height: usize, lines: &[String]) {
    for (line_index, line) in lines.iter().enumerate() {
        let y = TOP_MARGIN + line_index * LINE_ADVANCE;
        if y.saturating_add(GLYPH_HEIGHT) > height {
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
            || line.starts_with("ROUTE RECOVERY")
            || line.starts_with("NEW-PLAN")
            || line.starts_with("SAME-PLAN")
            || line.starts_with("UNPLANNED ROUTE")
            || line.starts_with("LINEAR NARRATION")
            || line.starts_with("ROUTE DETAIL")
        {
            ACCENT
        } else {
            FOREGROUND
        };
        let mut canvas = SoftwareCanvas::new(buffer, width, height);
        let _ = BitmapFont::draw_text(
            &mut canvas,
            Point::new(LEFT_MARGIN as i32, y as i32),
            line,
            softbuffer_to_rgb888(color),
        )
        .expect("software canvas drawing is infallible");
    }
}
