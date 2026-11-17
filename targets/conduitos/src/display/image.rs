//! Bounded opaque image rasterization using caller-held, admitted read views.
//! This module never opens resources or infers authority from a content key.
use super::*;
use conduit_face::GraphicsImageKey;

pub const MAX_IMAGE_PIXELS: usize = 1_048_576;
const SPAN_PIXELS: usize = 128;

#[derive(Clone, Copy)]
pub struct OpaqueRgbaImage<'a> {
    width: u16,
    height: u16,
    pixels: &'a [u8],
}

impl<'a> OpaqueRgbaImage<'a> {
    /// Validate immutable resident pixels during preparation. The caller owns
    /// the separate byte/work reservation and read lease for this borrow.
    pub fn prepare(width: u16, height: u16, pixels: &'a [u8]) -> Result<Self, DisplayError> {
        let count = usize::from(width) * usize::from(height);
        if count == 0 || count > MAX_IMAGE_PIXELS || pixels.len() != count * 4 {
            return Err(DisplayError::InvalidExtent);
        }
        if pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] != 255)
        {
            return Err(DisplayError::ImageUnsupported);
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }
}

/// Trusted boundary to independently admitted immutable image residence.
/// Implementations must validate current Plan/Boot/grant/lease and exact
/// generation, profile and extent before returning a view. A key grants none
/// of those facts. Expired or revoked access must return a distinct refusal.
pub trait ImageResolver {
    fn resolve(&self, key: GraphicsImageKey) -> Result<OpaqueRgbaImage<'_>, DisplayError>;
}

pub fn render_scene_with_images(
    target: &mut impl PixelTarget,
    scene: &GraphicsScene,
    images: &(impl ImageResolver + ?Sized),
) -> Result<DisplayReceipt, DisplayError> {
    let format = target.format().validate()?;
    let mut receipt = DisplayReceipt::default();
    for command in scene.commands() {
        if let Some(key) = command.image_key() {
            raster(target, command, images.resolve(key)?, &mut receipt)?;
        } else {
            render_command(target, format, command, &mut receipt)?;
        }
        receipt.commands += 1;
    }
    Ok(receipt)
}

/// Center-cover scaling is a bounded presentation choice, below image meaning.
pub(super) fn raster(
    target: &mut impl PixelTarget,
    command: &GraphicsCommand,
    image: OpaqueRgbaImage<'_>,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let format = target.format().validate()?;
    let Some(visible) = clipped(command.bounds, command.clip, format) else {
        return Ok(());
    };
    let sw = u64::from(image.width);
    let sh = u64::from(image.height);
    let dw = u64::from(command.bounds.width);
    let dh = u64::from(command.bounds.height);
    let (cw, ch) = if sw * dh > sh * dw {
        ((sh * dw / dh).max(1), sh)
    } else {
        (sw, (sw * dh / dw).max(1))
    };
    let left = (sw - cw) / 2;
    let top = (sh - ch) / 2;
    let mut span = [0; SPAN_PIXELS];
    for y in i32::from(visible.y)..i32::from(visible.y) + i32::from(visible.height) {
        let sy = top + (y - i32::from(command.bounds.y)) as u64 * ch / dh;
        let mut x = i32::from(visible.x);
        let end = x + i32::from(visible.width);
        while x < end {
            let count = usize::try_from(end - x)
                .map_err(|_| DisplayError::InvalidExtent)?
                .min(SPAN_PIXELS);
            for (index, pixel) in span[..count].iter_mut().enumerate() {
                let sx = left + (x + index as i32 - i32::from(command.bounds.x)) as u64 * cw / dw;
                let offset = ((sy * sw + sx) * 4) as usize;
                *pixel = format.pixel(
                    image.pixels[offset],
                    image.pixels[offset + 1],
                    image.pixels[offset + 2],
                );
            }
            target.write_span(x as u32, y as u32, &span[..count])?;
            receipt.pixels_written = receipt
                .pixels_written
                .checked_add(count as u32)
                .ok_or(DisplayError::InvalidExtent)?;
            x += count as i32;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
