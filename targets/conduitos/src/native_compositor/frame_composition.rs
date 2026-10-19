use crate::display::{DisplayError, DisplayFormat, PixelTarget};

use super::{
    CompositorSurface, DamageRect, FrameReceipt, MAX_DAMAGE_RECTS, NativeCompositor,
    NativeCompositorError,
};

impl NativeCompositor {
    pub fn compose_frame(
        &mut self,
        target: &mut impl PixelTarget,
    ) -> Result<FrameReceipt, NativeCompositorError> {
        // Invalidate before the first fallible operation: partial writes cannot
        // preserve acknowledgement of a prior successful frame.
        self.scanout_pixels = [0; super::MAX_COMPOSITOR_SURFACES];
        let format = target.format().validate()?;
        let mut damage_rects = [DamageRect::default(); MAX_DAMAGE_RECTS];
        let mut damage_count = 0_usize;
        for raw in self.damage.pending() {
            if let Some(clipped) = raw.clip(format.width, format.height) {
                damage_rects[damage_count] = clipped;
                damage_count += 1;
            }
        }
        let (pixels_written, surfaces_composed, scanout_pixels) = compose_damage(
            target,
            format,
            &self.surfaces,
            &damage_rects[..damage_count],
            self.focused_surface.as_deref(),
            self.cursor,
            self.cursor_hover,
        )?;
        self.frame_sequence = self
            .frame_sequence
            .checked_add(1)
            .ok_or(NativeCompositorError::Display(DisplayError::InvalidExtent))?;
        let receipt = FrameReceipt {
            frame_sequence: self.frame_sequence,
            surfaces_composed,
            pixels_written,
            damage_count: u8::try_from(damage_count)
                .map_err(|_| NativeCompositorError::SurfaceCapacityExceeded)?,
            damage_rects,
            conservative_fallback: self.damage.used_fallback(),
            cursor_visible: self.cursor.is_some(),
            focus_visible: self.focused_surface.is_some(),
        };
        for (index, surface) in self.surfaces.iter_mut().enumerate() {
            if scanout_pixels[index] > 0 {
                surface.input_ready = true;
            }
        }
        self.scanout_pixels = scanout_pixels;
        self.damage.clear();
        Ok(receipt)
    }
}

pub(super) fn compose_damage(
    target: &mut impl PixelTarget,
    format: DisplayFormat,
    surfaces: &[CompositorSurface],
    damage: &[DamageRect],
    focused_surface: Option<&str>,
    cursor: Option<(u32, u32)>,
    cursor_hover: bool,
) -> Result<(u32, u8, [u32; super::MAX_COMPOSITOR_SURFACES]), NativeCompositorError> {
    let focused = focused_surface.and_then(|id| {
        surfaces.iter().find(|surface| {
            surface.surface_id == id && surface.visible && surface.is_raster_ready()
        })
    });
    if let [rect] = damage
        && rect.width == 1
        && rect.height == 1
    {
        if rect.x >= format.width || rect.y >= format.height {
            return Err(NativeCompositorError::InvalidBounds);
        }
        if surfaces.len() > super::MAX_COMPOSITOR_SURFACES {
            return Err(NativeCompositorError::TooManySurfaces);
        }
        let selected = super::scanline::select_at(surfaces, rect.x, rect.y);
        let value = selected
            .map(|(_, surface)| surface_span(surface, rect.x, rect.x + 1, rect.y))
            .transpose()?
            .map_or(0, |pixels| pixels[0]);
        let (pixel, visible) = overlay(value, focused, cursor, cursor_hover, rect.x, rect.y);
        target.write_pixel(rect.x, rect.y, pixel)?;
        let mut composed = [0; super::MAX_COMPOSITOR_SURFACES];
        if visible && let Some((index, _)) = selected {
            composed[index] = 1;
        }
        return Ok((1, u8::from(selected.is_some()), composed));
    }
    let prepared = super::scanline::PreparedSurfaces::new(surfaces)?;
    let mut count = 0_u32;
    let mut selected_surfaces = [false; super::MAX_COMPOSITOR_SURFACES];
    let mut composed = [0_u32; super::MAX_COMPOSITOR_SURFACES];
    for rect in damage {
        let bottom = rect
            .y
            .checked_add(rect.height)
            .ok_or(NativeCompositorError::InvalidBounds)?;
        let right = rect
            .x
            .checked_add(rect.width)
            .ok_or(NativeCompositorError::InvalidBounds)?;
        if right > format.width || bottom > format.height {
            return Err(NativeCompositorError::InvalidBounds);
        }
        for y in rect.y..bottom {
            for (left, end, selected) in prepared.row(rect.x, right, y) {
                let pixels = selected
                    .map(|(_, surface)| surface_span(surface, left, end, y))
                    .transpose()?;
                if let Some((index, _)) = selected {
                    selected_surfaces[index] = true;
                }
                for x in left..end {
                    let (pixel, visible) = overlay(
                        pixels.map_or(0, |pixels| pixels[(x - left) as usize]),
                        focused,
                        cursor,
                        cursor_hover,
                        x,
                        y,
                    );
                    target.write_pixel(x, y, pixel)?;
                    // Overlay-only pixels do not demonstrate the surface itself.
                    if visible && let Some((index, _)) = selected {
                        composed[index] = composed[index]
                            .checked_add(1)
                            .ok_or(NativeCompositorError::Display(DisplayError::InvalidExtent))?;
                    }
                    count = count
                        .checked_add(1)
                        .ok_or(NativeCompositorError::Display(DisplayError::InvalidExtent))?;
                }
            }
        }
    }
    Ok((
        count,
        selected_surfaces
            .into_iter()
            .filter(|selected| *selected)
            .count() as u8,
        composed,
    ))
}

fn overlay(
    pixel: u32,
    focused: Option<&CompositorSurface>,
    cursor: Option<(u32, u32)>,
    hovered: bool,
    x: u32,
    y: u32,
) -> (u32, bool) {
    let focus = focused.is_some_and(|surface| focus_pixel(surface, x, y));
    let cursor_pixel = cursor.and_then(|position| cursor_color(position, x, y, hovered));
    (
        cursor_pixel.unwrap_or(if focus {
            crate::display::profile::FOCUS
        } else {
            pixel
        }),
        !focus && cursor_pixel.is_none(),
    )
}

fn focus_pixel(surface: &CompositorSurface, x: u32, y: u32) -> bool {
    let left = i64::from(surface.bounds.x);
    let top = i64::from(surface.bounds.y);
    let right = left + i64::from(surface.bounds.width) - 1;
    let bottom = top + i64::from(surface.bounds.height) - 1;
    let x = i64::from(x);
    let y = i64::from(y);
    (x == left || x == right || y == top || y == bottom)
        && x >= left
        && x <= right
        && y >= top
        && y <= bottom
}

const CURSOR_OUTLINE: [u16; 16] = [
    0x0001, 0x0003, 0x0005, 0x0009, 0x0011, 0x0021, 0x0041, 0x0081, 0x0101, 0x03e1, 0x0049, 0x0095,
    0x0093, 0x0121, 0x0120, 0x00c0,
];
const CURSOR_FILL: [u16; 16] = [
    0x0000, 0x0000, 0x0002, 0x0006, 0x000e, 0x001e, 0x003e, 0x007e, 0x00fe, 0x001e, 0x0036, 0x0062,
    0x0060, 0x00c0, 0x00c0, 0x0000,
];

fn cursor_color((cx, cy): (u32, u32), x: u32, y: u32, hovered: bool) -> Option<u32> {
    let (Some(dx), Some(dy)) = (x.checked_sub(cx), y.checked_sub(cy)) else {
        return None;
    };
    let row = usize::try_from(dy).ok()?;
    let bit = 1_u16.checked_shl(dx)?;
    if CURSOR_FILL.get(row).is_some_and(|mask| mask & bit != 0) {
        return Some(if hovered {
            crate::display::profile::HOVER
        } else {
            crate::display::profile::CURSOR
        });
    }
    CURSOR_OUTLINE
        .get(row)
        .is_some_and(|mask| mask & bit != 0)
        .then_some(0x00000000)
}

fn surface_span(
    surface: &CompositorSurface,
    left: u32,
    right: u32,
    y: u32,
) -> Result<&[u32], NativeCompositorError> {
    let local_x = i64::from(left) - i64::from(surface.bounds.x);
    let local_y = i64::from(y) - i64::from(surface.bounds.y);
    let index = usize::try_from(
        local_y
            .checked_mul(i64::from(surface.bounds.width))
            .and_then(|row| row.checked_add(local_x))
            .ok_or(NativeCompositorError::InvalidBounds)?,
    )
    .map_err(|_| NativeCompositorError::InvalidBounds)?;
    let end = index
        .checked_add((right - left) as usize)
        .ok_or(NativeCompositorError::InvalidBounds)?;
    surface
        .buffer
        .pixels
        .get(index..end)
        .ok_or(NativeCompositorError::InvalidBounds)
}

#[cfg(test)]
mod tests {
    use super::cursor_color;

    #[test]
    fn cursor_bitmap_has_outline_fill_stem_and_exact_hotspot() {
        let origin = (20, 30);
        assert_eq!(cursor_color(origin, 20, 30, false), Some(0x00000000));
        assert_eq!(cursor_color(origin, 21, 32, false), Some(0x00ffffff));
        assert_eq!(cursor_color(origin, 26, 44, true), Some(0x00ffcc33));
        assert_eq!(cursor_color(origin, 19, 30, false), None);
        assert_eq!(cursor_color(origin, 35, 46, false), None);
    }
}

#[cfg(test)]
#[path = "frame_composition_tests.rs"]
mod differential;
#[cfg(test)]
#[path = "frame_composition_reference.rs"]
mod reference;
