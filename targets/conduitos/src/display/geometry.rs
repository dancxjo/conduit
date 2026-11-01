use super::DisplayError;

/// Shared finite geometry admission; mapping and channel layout have their own owners.
pub(crate) fn validate_framebuffer_geometry(
    width: u32,
    height: u32,
    pitch_bytes: u32,
    bits_per_pixel: u8,
) -> Result<(), DisplayError> {
    let row_bytes = width.checked_mul(4).ok_or(DisplayError::InvalidExtent)?;
    if width == 0 || height == 0 || pitch_bytes < row_bytes {
        return Err(DisplayError::InvalidExtent);
    }
    if bits_per_pixel != 32 {
        return Err(DisplayError::UnsupportedFormat);
    }
    Ok(())
}
