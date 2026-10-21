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
        // Idle service calls still execute their ordinary acknowledgement path;
        // only requested damage consumes the finite progress observation budget.
        if self.damage.pending().is_empty() {
            return self.compose_frame_inner(target);
        }
        crate::native_observation::Progress::result(
            crate::native_observation::Phase::Scanout,
            "compositor",
            || self.compose_frame_inner(target),
        )
    }

    fn compose_frame_inner(
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
            let overlay_edges = overlay_edges(focused, cursor, y);
            for (left, end, selected) in prepared.row(rect.x, right, y, &overlay_edges) {
                let pixels = selected
                    .map(|(_, surface)| surface_span(surface, left, end, y))
                    .transpose()?;
                if let Some((index, _)) = selected {
                    selected_surfaces[index] = true;
                }
                let length = end - left;
                let coverage =
                    selected.map_or(Some(0), |(index, _)| composed[index].checked_add(length));
                if !overlay_intersects(focused, cursor, left, end, y)
                    && let (Some(next_count), Some(next_coverage)) =
                        (count.checked_add(length), coverage)
                {
                    if let Some(pixels) = pixels {
                        target.write_span(left, y, pixels)?;
                    } else {
                        write_black_span(target, left, end, y)?;
                    }
                    if let Some((index, _)) = selected {
                        composed[index] = next_coverage;
                    }
                    count = next_count;
                    continue;
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

fn write_black_span(
    target: &mut impl PixelTarget,
    mut left: u32,
    right: u32,
    y: u32,
) -> Result<(), DisplayError> {
    const BLACK: [u32; 64] = [0; 64];
    while left < right {
        let length = (right - left).min(BLACK.len() as u32);
        target.write_span(left, y, &BLACK[..length as usize])?;
        left += length;
    }
    Ok(())
}

fn overlay_edges(
    focused: Option<&CompositorSurface>,
    cursor: Option<(u32, u32)>,
    y: u32,
) -> [Option<u32>; super::scanline::MAX_OVERLAY_EDGES] {
    let mut edges = [None; super::scanline::MAX_OVERLAY_EDGES];
    if let Some(surface) = focused {
        let bounds = surface.bounds;
        if i64::from(y) >= i64::from(bounds.y)
            && i64::from(y) < i64::from(bounds.y) + i64::from(bounds.height)
        {
            let left = i64::from(bounds.x);
            let right = left + i64::from(bounds.width);
            for (slot, edge) in edges.iter_mut().zip([left, left + 1, right - 1, right]) {
                *slot = u32::try_from(edge).ok();
            }
        }
    }
    if let Some((x, cy)) = cursor
        && y >= cy
        && u64::from(y) < u64::from(cy) + 16
    {
        edges[4] = Some(x);
        edges[5] = u32::try_from(u64::from(x) + 16).ok();
    }
    edges
}

fn overlay_intersects(
    focused: Option<&CompositorSurface>,
    cursor: Option<(u32, u32)>,
    left: u32,
    right: u32,
    y: u32,
) -> bool {
    let focus = focused.is_some_and(|surface| {
        let bounds = surface.bounds;
        let x0 = i64::from(bounds.x);
        let x1 = x0 + i64::from(bounds.width) - 1;
        let y0 = i64::from(bounds.y);
        let y1 = y0 + i64::from(bounds.height) - 1;
        let left = i64::from(left);
        let right = i64::from(right);
        let y = i64::from(y);
        y >= y0
            && y <= y1
            && (((y == y0 || y == y1) && left <= x1 && right > x0)
                || (left <= x0 && x0 < right)
                || (left <= x1 && x1 < right))
    });
    focus
        || cursor.is_some_and(|(x, cy)| {
            y >= cy
                && u64::from(y) < u64::from(cy) + 16
                && u64::from(left) < u64::from(x) + 16
                && right > x
        })
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
