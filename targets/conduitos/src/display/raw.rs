//! Checked scanout access, with explicit device permission for pixel-word stores.
use super::{DisplayError, DisplayFormat, PixelTarget};
use core::ptr::NonNull;

pub struct RawDisplay {
    address: NonNull<u8>,
    byte_len: usize,
    format: DisplayFormat,
    available: bool,
    word_writable: bool,
}

impl RawDisplay {
    /// # Safety
    ///
    /// `address..address + byte_len` must remain uniquely writable display
    /// memory for this value's lifetime. The caller owns synchronization.
    pub unsafe fn new(
        address: NonNull<u8>,
        byte_len: usize,
        format: DisplayFormat,
    ) -> Result<Self, DisplayError> {
        let format = format.validate()?;
        if byte_len < format.byte_len()? {
            return Err(DisplayError::BufferTooSmall);
        }
        Ok(Self {
            address,
            byte_len,
            format,
            available: true,
            word_writable: false,
        })
    }

    /// Prepare scanout on a Base that accepts native pixel-word writes.
    /// Unaligned addresses still use the byte path.
    ///
    /// # Safety
    /// The memory must satisfy `new` and permit aligned 32-bit volatile stores
    /// in addition to byte stores throughout its lifetime.
    pub unsafe fn new_word_writable(
        address: NonNull<u8>,
        byte_len: usize,
        format: DisplayFormat,
    ) -> Result<Self, DisplayError> {
        // SAFETY: The caller supplies the stronger scanout access contract.
        let mut display = unsafe { Self::new(address, byte_len, format)? };
        display.word_writable = true;
        Ok(display)
    }

    /// # Safety
    /// The complete four-byte pixel at `offset` must be inside this Base.
    unsafe fn write_at(&mut self, offset: usize, pixel: u32) {
        // SAFETY: The caller has checked this pixel's admitted byte extent.
        let output = unsafe { self.address.as_ptr().add(offset) };
        if self.word_writable && output.cast::<u32>().is_aligned() {
            // SAFETY: Alignment was checked; the constructor admits word stores.
            unsafe { core::ptr::write_volatile(output.cast::<u32>(), pixel.to_le()) };
        } else {
            for (index, byte) in pixel.to_le_bytes().into_iter().enumerate() {
                // SAFETY: Every byte belongs to the checked four-byte pixel.
                unsafe { core::ptr::write_volatile(output.add(index), byte) };
            }
        }
    }

    pub fn lose(&mut self) {
        self.available = false;
    }
}

impl PixelTarget for RawDisplay {
    fn format(&self) -> DisplayFormat {
        self.format
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if !self.available {
            return Err(DisplayError::Lost);
        }
        let offset = usize::try_from(
            u64::from(y)
                .checked_mul(u64::from(self.format.pitch))
                .and_then(|row| row.checked_add(u64::from(x) * 4))
                .ok_or(DisplayError::InvalidExtent)?,
        )
        .map_err(|_| DisplayError::InvalidExtent)?;
        if offset.checked_add(4).is_none_or(|end| end > self.byte_len) {
            return Err(DisplayError::BufferTooSmall);
        }
        // SAFETY: The pixel extent was checked against the owned Base above.
        unsafe { self.write_at(offset, pixel) };
        Ok(())
    }
    fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
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
        if !self.available {
            return Err(DisplayError::Lost);
        }
        let offset = usize::try_from(
            u64::from(y)
                .checked_mul(u64::from(self.format.pitch))
                .and_then(|row| row.checked_add(u64::from(x) * 4))
                .ok_or(DisplayError::InvalidExtent)?,
        )
        .map_err(|_| DisplayError::InvalidExtent)?;
        let bytes = pixels
            .len()
            .checked_mul(4)
            .ok_or(DisplayError::InvalidExtent)?;
        if offset
            .checked_add(bytes)
            .is_none_or(|end| end > self.byte_len)
        {
            return Err(DisplayError::BufferTooSmall);
        }
        for (index, pixel) in pixels.iter().enumerate() {
            // SAFETY: The entire span is in one checked row inside this Base.
            unsafe { self.write_at(offset + index * 4, *pixel) };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
