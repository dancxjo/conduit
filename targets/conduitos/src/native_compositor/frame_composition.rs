use crate::display::{DisplayError, DisplayFormat, PixelTarget};

use super::{CompositorSurface, NativeCompositorError};

pub(super) fn clear_target(
    target: &mut impl PixelTarget,
    format: DisplayFormat,
) -> Result<u32, NativeCompositorError> {
    let mut count = 0_u32;
    for y in 0..format.height {
        for x in 0..format.width {
            target.write_pixel(x, y, 0)?;
            count = count
                .checked_add(1)
                .ok_or(NativeCompositorError::Display(DisplayError::InvalidExtent))?;
        }
    }
    Ok(count)
}

pub(super) fn blit_surface(
    target: &mut impl PixelTarget,
    target_format: DisplayFormat,
    surface: &CompositorSurface,
) -> Result<u32, NativeCompositorError> {
    let mut count = 0_u32;
    for local_y in 0..u32::from(surface.bounds.height) {
        let y = i32::from(surface.bounds.y) + i32::try_from(local_y).unwrap_or(i32::MAX);
        if y < 0 || y >= i32::try_from(target_format.height).unwrap_or(i32::MAX) {
            continue;
        }
        for local_x in 0..u32::from(surface.bounds.width) {
            let x = i32::from(surface.bounds.x) + i32::try_from(local_x).unwrap_or(i32::MAX);
            if x < 0 || x >= i32::try_from(target_format.width).unwrap_or(i32::MAX) {
                continue;
            }
            let index = usize::try_from(local_y * u32::from(surface.bounds.width) + local_x)
                .map_err(|_| NativeCompositorError::InvalidBounds)?;
            target.write_pixel(
                u32::try_from(x).map_err(|_| NativeCompositorError::InvalidBounds)?,
                u32::try_from(y).map_err(|_| NativeCompositorError::InvalidBounds)?,
                surface.buffer.pixels[index],
            )?;
            count = count
                .checked_add(1)
                .ok_or(NativeCompositorError::Display(DisplayError::InvalidExtent))?;
        }
    }
    Ok(count)
}
