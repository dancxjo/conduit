//! One caller-owned transient raster, not retained compositor state or authority.
//! Admission borrows exactly 8 MiB once. Preparation/render/upload allocate no
//! storage here. The guest alone retains committed rasters after the lease ends.
use crate::display::{DisplayError, DisplayFormat, PixelTarget, RetainedPixelTarget};

pub const STAGING_PIXELS: usize = 2_097_152;
pub const UPLOAD_CHUNK_PIXELS: usize = 16_384;

pub struct RasterStaging<'a> {
    pixels: &'a mut [u32],
}
impl<'a> RasterStaging<'a> {
    /// Validate capacity before touching the caller's buffer. No allocation.
    pub fn admit(pixels: &'a mut [u32]) -> Result<Self, DisplayError> {
        if pixels.len() != STAGING_PIXELS {
            return Err(DisplayError::BufferTooSmall);
        }
        pixels.fill(0);
        Ok(Self { pixels })
    }

    /// Exclusively lease an exact, tightly packed canonical RGB raster. There
    /// is no resize in flight: drop this lease before admitting another extent.
    /// Clear the selected extent even if a previous lease was safely forgotten.
    pub fn lease(&mut self, width: u16, height: u16) -> Result<RasterLease<'_>, DisplayError> {
        let format = DisplayFormat {
            width: u32::from(width),
            height: u32::from(height),
            pitch: u32::from(width)
                .checked_mul(4)
                .ok_or(DisplayError::InvalidExtent)?,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
        .validate()?;
        let used = usize::from(width)
            .checked_mul(usize::from(height))
            .ok_or(DisplayError::InvalidExtent)?;
        let pixels = self
            .pixels
            .get_mut(..used)
            .ok_or(DisplayError::BufferTooSmall)?;
        pixels.fill(0);
        Ok(RasterLease { pixels, format })
    }
}

/// Borrowed bytes exist only for this lease; upload must complete or abort before
/// Drop clears them. This helper does not admit a surface, request, or service.
/// A caller must not forget the lease when it requires cleanup on scope exit.
pub struct RasterLease<'a> {
    pixels: &'a mut [u32],
    format: DisplayFormat,
}
impl RasterLease<'_> {
    #[cfg(test)]
    pub fn pixel_count(&self) -> usize {
        self.pixels.len()
    }

    /// Row-major canonical RGB words, each chunk <= 64 KiB. The immutable borrow
    /// excludes writes/lease release while a caller uploads that chunk. These
    /// are slices of the sole staging arena, never a second raster or snapshot.
    pub fn upload_chunks(&self) -> impl ExactSizeIterator<Item = &[u32]> {
        self.pixels.chunks(UPLOAD_CHUNK_PIXELS)
    }
    fn index(&self, x: u32, y: u32) -> Result<usize, DisplayError> {
        if x >= self.format.width || y >= self.format.height {
            return Err(DisplayError::InvalidExtent);
        }
        usize::try_from(u64::from(y) * u64::from(self.format.width) + u64::from(x))
            .map_err(|_| DisplayError::InvalidExtent)
    }
}
impl PixelTarget for RasterLease<'_> {
    fn format(&self) -> DisplayFormat {
        self.format
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        let index = self.index(x, y)?;
        self.pixels[index] = pixel;
        Ok(())
    }
    fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
        // Match the PixelTarget contract: empty spans have no effect.
        if pixels.is_empty() {
            return Ok(());
        }
        let count = u32::try_from(pixels.len()).map_err(|_| DisplayError::InvalidExtent)?;
        if y >= self.format.height
            || x.checked_add(count)
                .is_none_or(|end| end > self.format.width)
        {
            return Err(DisplayError::InvalidExtent);
        }
        let index = self.index(x, y)?;
        self.pixels[index..index + pixels.len()].copy_from_slice(pixels);
        Ok(())
    }
}
impl RetainedPixelTarget for RasterLease<'_> {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        Ok(self.pixels[self.index(x, y)?])
    }
}
impl Drop for RasterLease<'_> {
    fn drop(&mut self) {
        self.pixels.fill(0);
    }
}

#[cfg(test)]
#[path = "raster_staging/tests.rs"]
mod tests;
