//! Owned finite raster using the existing portable GraphicsScene renderer.
use conduit_face::GraphicsScene;
use conduitos::display::{render_scene, DisplayError, DisplayFormat, DisplayReceipt, PixelTarget};
const MAXIMUM_SURFACE_BYTES: usize = 8 * 1024 * 1024;
pub struct Surface {
    format: DisplayFormat,
    bytes: Vec<u8>,
    live: bool,
}
impl Surface {
    pub fn prepare(width: u32, height: u32) -> Result<Self, &'static str> {
        let format = DisplayFormat {
            width,
            height,
            pitch: width.checked_mul(4).ok_or("display-extent-invalid")?,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
        .validate()
        .map_err(DisplayError::as_str)?;
        let length = format.byte_len().map_err(DisplayError::as_str)?;
        if length > MAXIMUM_SURFACE_BYTES {
            return Err("display-surface-capacity");
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| "display-surface-allocation")?;
        bytes.resize(length, 0);
        Ok(Self {
            format,
            bytes,
            live: true,
        })
    }
    pub fn render(&mut self, scene: &GraphicsScene) -> Result<DisplayReceipt, &'static str> {
        if !self.live {
            return Err("display-custody-lost");
        }
        // A scanout is opaque. SDL ARGB8888 must not interpret the raster's
        // unused high byte as transparency when copying to its window target.
        for pixel in self.bytes.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&0xff00_0000_u32.to_ne_bytes());
        }
        render_scene(self, scene).map_err(DisplayError::as_str)
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn pitch(&self) -> usize {
        self.format.pitch as usize
    }
    pub fn lose(&mut self) {
        self.live = false;
        self.bytes.fill(0);
    }
}
impl PixelTarget for Surface {
    fn format(&self) -> DisplayFormat {
        self.format
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if !self.live {
            return Err(DisplayError::Lost);
        }
        if x >= self.format.width || y >= self.format.height {
            return Err(DisplayError::InvalidExtent);
        }
        let offset = y as usize * self.pitch() + x as usize * 4;
        // SDL's packed ARGB8888 follows native word byte order.
        self.bytes[offset..offset + 4].copy_from_slice(&(pixel | 0xff00_0000).to_ne_bytes());
        Ok(())
    }
}
