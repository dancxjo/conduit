use alloc::{vec, vec::Vec};

use crate::display::{DisplayError, DisplayFormat, PixelTarget};

use super::{MAX_COMPOSITOR_PIXELS, MAX_COMPOSITOR_SURFACES, NativeCompositorError};
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

pub(super) struct SurfaceBufferPool {
    allocated_pixels: usize,
    available: Vec<SurfaceBuffer>,
}

impl SurfaceBufferPool {
    pub(super) fn new() -> Self {
        Self {
            allocated_pixels: 0,
            available: Vec::with_capacity(MAX_COMPOSITOR_SURFACES),
        }
    }

    pub(super) const fn allocated_pixels(&self) -> usize {
        self.allocated_pixels
    }

    pub(super) fn take(
        &mut self,
        bounds: LayoutRect,
    ) -> Result<SurfaceBuffer, NativeCompositorError> {
        if let Some(index) = self
            .available
            .iter()
            .position(|buffer| buffer.matches(bounds.width, bounds.height))
        {
            return Ok(self.available.swap_remove(index));
        }
        let pixels = surface_pixels(bounds)?;
        let allocated_pixels = self
            .allocated_pixels
            .checked_add(pixels)
            .ok_or(NativeCompositorError::SurfaceCapacityExceeded)?;
        if allocated_pixels > MAX_COMPOSITOR_PIXELS {
            return Err(NativeCompositorError::SurfaceCapacityExceeded);
        }
        let buffer = SurfaceBuffer::new(bounds.width, bounds.height)?;
        self.allocated_pixels = allocated_pixels;
        Ok(buffer)
    }

    pub(super) fn retain(&mut self, buffer: SurfaceBuffer) {
        if self.available.len() < MAX_COMPOSITOR_SURFACES {
            self.available.push(buffer);
        }
    }
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

    pub(super) fn matches(&self, width: u16, height: u16) -> bool {
        self.format.width == u32::from(width) && self.format.height == u32::from(height)
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
