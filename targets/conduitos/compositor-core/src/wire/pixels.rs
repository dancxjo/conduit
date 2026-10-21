use super::*;
/// Borrowed canonical LE pixels, independent of native alignment/endianness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelBytes<'a>(&'a [u8]);
impl<'a> PixelBytes<'a> {
    pub fn bytes(self) -> &'a [u8] {
        self.0
    }
    pub fn pixels(self) -> impl Iterator<Item = u32> + 'a {
        self.0
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
    }
}
/// The window direction belongs to one admitted request; Root and service do
/// not execute concurrently. No mutable borrow is retained across a core call.
pub fn decode_pixels(
    window: &[u8],
    offset: u32,
    pixels: u32,
) -> Result<PixelBytes<'_>, CodecError> {
    window_range(offset, pixels)?;
    if window.len() > OUTPUT_BYTES as usize {
        return Err(CodecError::Length);
    }
    let start = usize::try_from(offset).map_err(|_| CodecError::Length)?;
    let count = usize::try_from(pixels)
        .map_err(|_| CodecError::Length)?
        .checked_mul(4)
        .ok_or(CodecError::Length)?;
    let end = start.checked_add(count).ok_or(CodecError::Length)?;
    Ok(PixelBytes(
        window.get(start..end).ok_or(CodecError::Truncated)?,
    ))
}
pub fn encode_pixels(window: &mut [u8], offset: u32, pixels: &[u32]) -> Result<(), CodecError> {
    let count = u32::try_from(pixels.len()).map_err(|_| CodecError::Length)?;
    window_range(offset, count)?;
    if window.len() > OUTPUT_BYTES as usize {
        return Err(CodecError::Length);
    }
    let start = usize::try_from(offset).map_err(|_| CodecError::Length)?;
    let bytes = pixels.len().checked_mul(4).ok_or(CodecError::Length)?;
    let end = start.checked_add(bytes).ok_or(CodecError::Length)?;
    let output = window.get_mut(start..end).ok_or(CodecError::Truncated)?;
    for (chunk, pixel) in output.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
        chunk.copy_from_slice(&pixel.to_le_bytes());
    }
    Ok(())
}
