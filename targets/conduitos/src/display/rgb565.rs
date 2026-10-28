//! Bounded packing of the renderer's `0x00RRGGBB` raster into RGB565 bytes.
//!
//! Orientation describes only a software transform of the source raster.
//! Controller addressing and the orientation of a physical panel remain
//! responsibilities of the caller.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Orientation {
    Native,
    Rotate180,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rgb565Error {
    EmptyGeometry,
    GeometryOverflow,
    PixelLengthMismatch,
    PixelRangeOutOfBounds,
    ByteLengthOverflow,
    OutputTooShort,
}

/// Borrowed, exact row-major renderer output; dimensions cannot change after admission.
#[derive(Clone, Copy)]
pub struct Rgb565Raster<'a> {
    pixels: &'a [u32],
    width: u32,
    height: u32,
}

impl<'a> Rgb565Raster<'a> {
    pub fn new(pixels: &'a [u32], width: u32, height: u32) -> Result<Self, Rgb565Error> {
        if width == 0 || height == 0 {
            return Err(Rgb565Error::EmptyGeometry);
        }
        let count = width
            .checked_mul(height)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or(Rgb565Error::GeometryOverflow)?;
        if pixels.len() != count {
            return Err(Rgb565Error::PixelLengthMismatch);
        }
        Ok(Self {
            pixels,
            width,
            height,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn pixel_count(&self) -> usize {
        self.pixels.len()
    }

    /// Pack an exact chunk of the oriented row-major raster, without allocation.
    ///
    /// Offsets count output pixels, including when rotated. RGB channels truncate
    /// to 5/6/5 high bits; the unused high source byte is ignored. All bounds are
    /// checked before writing, so refusal leaves the entire output unchanged.
    /// A larger output is permitted, including an odd spare byte; its tail is
    /// untouched. A zero-length chunk at the end of the raster is permitted.
    pub fn encode_into(
        &self,
        pixel_offset: usize,
        pixel_count: usize,
        orientation: Orientation,
        output: &mut [u8],
    ) -> Result<usize, Rgb565Error> {
        let end = pixel_offset
            .checked_add(pixel_count)
            .filter(|end| *end <= self.pixels.len())
            .ok_or(Rgb565Error::PixelRangeOutOfBounds)?;
        let bytes = pixel_count
            .checked_mul(2)
            .ok_or(Rgb565Error::ByteLengthOverflow)?;
        if output.len() < bytes {
            return Err(Rgb565Error::OutputTooShort);
        }
        for (index, pixel_index) in (pixel_offset..end).enumerate() {
            let source = match orientation {
                Orientation::Native => pixel_index,
                Orientation::Rotate180 => self.pixels.len() - 1 - pixel_index,
            };
            let rgb = self.pixels[source];
            let packed =
                (((rgb >> 19) & 0x1f) << 11) | (((rgb >> 10) & 0x3f) << 5) | ((rgb >> 3) & 0x1f);
            let pair = (packed as u16).to_be_bytes();
            output[index * 2..index * 2 + 2].copy_from_slice(&pair);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_primary_colors_msb_first_and_truncates_channels() {
        let pixels = [
            0x00ff0000, 0x0000ff00, 0x000000ff, 0x00ffffff, 0x00000000,
            0xab070307, // Bits discarded by all three channels, plus ignored high byte.
            0x00080408, // First representable step of each channel.
        ];
        let raster = Rgb565Raster::new(&pixels, 7, 1).unwrap();
        let mut bytes = [0; 14];
        assert_eq!(
            raster.encode_into(0, 7, Orientation::Native, &mut bytes),
            Ok(14)
        );
        assert_eq!(
            bytes,
            [
                0xf8, 0x00, 0x07, 0xe0, 0x00, 0x1f, 0xff, 0xff, 0, 0, 0, 0, 0x08, 0x21
            ]
        );
    }

    #[test]
    fn rejects_inexact_or_overflowing_geometry() {
        assert!(matches!(
            Rgb565Raster::new(&[], 0, 1),
            Err(Rgb565Error::EmptyGeometry)
        ));
        assert!(matches!(
            Rgb565Raster::new(&[], 1, 0),
            Err(Rgb565Error::EmptyGeometry)
        ));
        assert!(matches!(
            Rgb565Raster::new(&[], u32::MAX, 2),
            Err(Rgb565Error::GeometryOverflow)
        ));
        assert!(matches!(
            Rgb565Raster::new(&[0; 3], 2, 2),
            Err(Rgb565Error::PixelLengthMismatch)
        ));
        assert!(matches!(
            Rgb565Raster::new(&[0; 5], 2, 2),
            Err(Rgb565Error::PixelLengthMismatch)
        ));
    }

    #[test]
    fn chunk_bounds_and_short_output_refuse_before_writing() {
        let raster = Rgb565Raster::new(&[0xff0000, 0x00ff00, 0x0000ff], 3, 1).unwrap();
        let mut output = [0xa5; 5];
        assert_eq!(
            raster.encode_into(0, 3, Orientation::Native, &mut output),
            Err(Rgb565Error::OutputTooShort)
        );
        assert_eq!(
            raster.encode_into(2, 2, Orientation::Native, &mut output),
            Err(Rgb565Error::PixelRangeOutOfBounds)
        );
        assert_eq!(
            raster.encode_into(usize::MAX, 1, Orientation::Native, &mut output),
            Err(Rgb565Error::PixelRangeOutOfBounds)
        );
        assert_eq!(
            raster.encode_into(4, 0, Orientation::Native, &mut output),
            Err(Rgb565Error::PixelRangeOutOfBounds)
        );
        assert_eq!(output, [0xa5; 5]);
        assert_eq!(
            raster.encode_into(1, 2, Orientation::Native, &mut output),
            Ok(4)
        );
        assert_eq!(output, [0x07, 0xe0, 0x00, 0x1f, 0xa5]);
        assert_eq!(
            raster.encode_into(3, 0, Orientation::Native, &mut output),
            Ok(0)
        );
        assert_eq!(output, [0x07, 0xe0, 0x00, 0x1f, 0xa5]);
    }

    #[test]
    fn rotation_reverses_both_axes_and_chunk_offsets_follow_output_order() {
        let pixels = [0xff0000, 0x00ff00, 0x0000ff, 0xffffff, 0x000000, 0x080408];
        let raster = Rgb565Raster::new(&pixels, 3, 2).unwrap();
        assert_eq!(
            (raster.width(), raster.height(), raster.pixel_count()),
            (3, 2, 6)
        );
        let mut full = [0; 12];
        raster
            .encode_into(0, 6, Orientation::Rotate180, &mut full)
            .unwrap();
        assert_eq!(
            full,
            [0x08, 0x21, 0, 0, 0xff, 0xff, 0, 0x1f, 0x07, 0xe0, 0xf8, 0]
        );
        let mut chunk = [0; 6];
        raster
            .encode_into(1, 3, Orientation::Rotate180, &mut chunk)
            .unwrap();
        assert_eq!(chunk, full[2..8]);
    }
}
