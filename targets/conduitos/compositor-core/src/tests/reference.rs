//! Frozen independent per-pixel oracle adapted from native_compositor/
//! frame_composition_reference.rs; cursor masks and overlay precedence retained.
use crate::Rect;
use std::{vec, vec::Vec};
pub struct Surface {
    pub rect: Rect,
    pub z: u8,
    pub visible: bool,
    pub pixels: Vec<u32>,
}
pub fn compose(
    surfaces: &[Surface],
    width: usize,
    height: usize,
    focus: Option<usize>,
    cursor: Option<(u16, u16)>,
    hover: bool,
) -> (Vec<u32>, [u32; 8]) {
    let mut output = vec![0; width * height];
    let mut coverage = [0; 8];
    for y in 0..height {
        for x in 0..width {
            let selected = surfaces
                .iter()
                .enumerate()
                .filter(|(_, s)| s.visible && contains(s.rect, x, y))
                .max_by_key(|(i, s)| (s.z, *i));
            let mut pixel = selected.map_or(0, |(_, s)| {
                s.pixels[(y as i64 - i64::from(s.rect.y)) as usize * usize::from(s.rect.width)
                    + (x as i64 - i64::from(s.rect.x)) as usize]
            });
            let focused = focus.is_some_and(|i| {
                let s = &surfaces[i];
                let r = s.rect;
                s.visible
                    && contains(r, x, y)
                    && (x as i64 == i64::from(r.x)
                        || x as i64 == i64::from(r.x) + i64::from(r.width) - 1
                        || y as i64 == i64::from(r.y)
                        || y as i64 == i64::from(r.y) + i64::from(r.height) - 1)
            });
            let overlay = cursor.and_then(|p| cursor_color(p, x as u32, y as u32, hover));
            if focused {
                pixel = 0x00ffcc33;
            }
            if let Some(color) = overlay {
                pixel = color;
            }
            if !focused
                && overlay.is_none()
                && let Some((i, _)) = selected
            {
                coverage[i] += 1;
            }
            output[y * width + x] = pixel;
        }
    }
    (output, coverage)
}
fn contains(r: Rect, x: usize, y: usize) -> bool {
    let (x, y) = (x as i64, y as i64);
    x >= i64::from(r.x)
        && x < i64::from(r.x) + i64::from(r.width)
        && y >= i64::from(r.y)
        && y < i64::from(r.y) + i64::from(r.height)
}
const CURSOR_OUTLINE: [u16; 16] = [
    0x0001, 0x0003, 0x0005, 0x0009, 0x0011, 0x0021, 0x0041, 0x0081, 0x0101, 0x03e1, 0x0049, 0x0095,
    0x0093, 0x0121, 0x0120, 0x00c0,
];
const CURSOR_FILL: [u16; 16] = [
    0x0000, 0x0000, 0x0002, 0x0006, 0x000e, 0x001e, 0x003e, 0x007e, 0x00fe, 0x001e, 0x0036, 0x0062,
    0x0060, 0x00c0, 0x00c0, 0x0000,
];

fn cursor_color((cx, cy): (u16, u16), x: u32, y: u32, hovered: bool) -> Option<u32> {
    let (Some(dx), Some(dy)) = (x.checked_sub(u32::from(cx)), y.checked_sub(u32::from(cy))) else {
        return None;
    };
    let row = usize::try_from(dy).ok()?;
    let bit = 1_u16.checked_shl(dx)?;
    if CURSOR_FILL.get(row).is_some_and(|mask| mask & bit != 0) {
        return Some(if hovered { 0x00ffcc33 } else { 0x00ffffff });
    }
    CURSOR_OUTLINE
        .get(row)
        .is_some_and(|mask| mask & bit != 0)
        .then_some(0x00000000)
}
