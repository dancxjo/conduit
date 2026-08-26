//! Portable finite raster Info, independent of any presentation mechanism.

use alloc::{vec, vec::Vec};

pub const GRAY8_BITMAP_INFO_KIND: &str = "graphics/bitmap-gray8@1";
pub const MAX_BITMAP_WIDTH: u16 = 128;
pub const MAX_BITMAP_HEIGHT: u16 = 128;
pub const MAX_BITMAP_PIXELS: usize = MAX_BITMAP_WIDTH as usize * MAX_BITMAP_HEIGHT as usize;
pub const MAX_GRAY8_BITMAP_BYTES: usize = BITMAP_HEADER_BYTES + MAX_BITMAP_PIXELS;

const BITMAP_MAGIC: [u8; 8] = *b"CNDBMP01";
const BITMAP_HEADER_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gray8Bitmap {
    width: u16,
    height: u16,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapRefusal {
    InvalidExtent,
    PixelCountMismatch,
    WrongMagic,
    MalformedEncoding,
    NonCanonicalEncoding,
}

impl Gray8Bitmap {
    pub fn new(width: u16, height: u16, pixels: Vec<u8>) -> Result<Self, BitmapRefusal> {
        let expected = checked_pixel_count(width, height)?;
        if pixels.len() != expected {
            return Err(BitmapRefusal::PixelCountMismatch);
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn encoded_len(&self) -> usize {
        BITMAP_HEADER_BYTES + self.pixels.len()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut output = vec![0; self.encoded_len()];
        output[..8].copy_from_slice(&BITMAP_MAGIC);
        output[8..10].copy_from_slice(&self.width.to_le_bytes());
        output[10..12].copy_from_slice(&self.height.to_le_bytes());
        output[12..16].copy_from_slice(&(self.pixels.len() as u32).to_le_bytes());
        output[BITMAP_HEADER_BYTES..].copy_from_slice(&self.pixels);
        output
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, BitmapRefusal> {
        if encoded.len() < BITMAP_HEADER_BYTES {
            return Err(BitmapRefusal::MalformedEncoding);
        }
        if encoded[..8] != BITMAP_MAGIC {
            return Err(BitmapRefusal::WrongMagic);
        }
        let width = u16::from_le_bytes([encoded[8], encoded[9]]);
        let height = u16::from_le_bytes([encoded[10], encoded[11]]);
        let declared = u32::from_le_bytes([encoded[12], encoded[13], encoded[14], encoded[15]]);
        let expected = checked_pixel_count(width, height)?;
        if usize::try_from(declared).ok() != Some(expected) {
            return Err(BitmapRefusal::PixelCountMismatch);
        }
        if encoded.len() != BITMAP_HEADER_BYTES + expected {
            return Err(BitmapRefusal::NonCanonicalEncoding);
        }
        Self::new(width, height, encoded[BITMAP_HEADER_BYTES..].to_vec())
    }
}

fn checked_pixel_count(width: u16, height: u16) -> Result<usize, BitmapRefusal> {
    if width == 0 || height == 0 || width > MAX_BITMAP_WIDTH || height > MAX_BITMAP_HEIGHT {
        return Err(BitmapRefusal::InvalidExtent);
    }
    Ok(usize::from(width) * usize::from(height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_exact_and_bounded() {
        let bitmap = Gray8Bitmap::new(2, 2, vec![0, 64, 128, 255]).unwrap();
        let encoded = bitmap.encode();
        assert_eq!(Gray8Bitmap::decode(&encoded), Ok(bitmap));
        assert!(encoded.len() <= MAX_GRAY8_BITMAP_BYTES);
    }

    #[test]
    fn malformed_extent_count_and_trailing_data_refuse_distinctly() {
        assert_eq!(
            Gray8Bitmap::new(0, 1, vec![]),
            Err(BitmapRefusal::InvalidExtent)
        );
        assert_eq!(
            Gray8Bitmap::new(2, 2, vec![0; 3]),
            Err(BitmapRefusal::PixelCountMismatch)
        );
        let mut encoded = Gray8Bitmap::new(1, 1, vec![7]).unwrap().encode();
        encoded.push(0);
        assert_eq!(
            Gray8Bitmap::decode(&encoded),
            Err(BitmapRefusal::NonCanonicalEncoding)
        );
    }
}
