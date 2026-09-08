use alloc::{vec, vec::Vec};

use crate::display::{DisplayError, DisplayFormat, PixelTarget};

use super::NativeCompositorError;
use conduit_presentation::LayoutRect;

pub(super) fn surface_pixels(bounds: LayoutRect) -> Result<usize, NativeCompositorError> {
    if bounds.width == 0 || bounds.height == 0 {
        return Err(NativeCompositorError::InvalidBounds);
    }
    usize::from(bounds.width)
        .checked_mul(usize::from(bounds.height))
        .ok_or(NativeCompositorError::InvalidBounds)
}

pub(super) struct SurfaceBuffer {
    format: DisplayFormat,
    pub(super) pixels: Vec<u32>,
}

impl SurfaceBuffer {
    pub(super) fn new(width: u16, height: u16) -> Result<Self, NativeCompositorError> {
        let len = usize::from(width)
            .checked_mul(usize::from(height))
            .ok_or(NativeCompositorError::InvalidBounds)?;
        let format = DisplayFormat {
            width: u32::from(width),
            height: u32::from(height),
            pitch: u32::from(width)
                .checked_mul(4)
                .ok_or(NativeCompositorError::InvalidBounds)?,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
        .validate()?;
        Ok(Self {
            format,
            pixels: vec![0; len],
        })
    }

    pub(super) fn clear(&mut self) {
        self.pixels.fill(0);
    }
}

impl PixelTarget for SurfaceBuffer {
    fn format(&self) -> DisplayFormat {
        self.format
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        let index = usize::try_from(
            u64::from(y)
                .checked_mul(u64::from(self.format.width))
                .and_then(|row| row.checked_add(u64::from(x)))
                .ok_or(DisplayError::InvalidExtent)?,
        )
        .map_err(|_| DisplayError::InvalidExtent)?;
        *self
            .pixels
            .get_mut(index)
            .ok_or(DisplayError::BufferTooSmall)? = pixel;
        Ok(())
    }
}
