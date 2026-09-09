//! Finite framebuffer mechanism below portable graphics meaning.

mod font;
mod text_layout;
mod tokens;
pub use tokens::*;
#[cfg(any(test, feature = "native-compositor"))]
pub mod profile;
#[cfg(feature = "native-compositor")]
pub mod typography;
pub(crate) fn text_height(value: &str, width: u16) -> Result<u16, DisplayError> {
    text_layout::text_height(value, width)
}

use conduit_presentation::{
    GraphicsCommand, GraphicsCommandKind, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle,
    LayoutRect,
};
use core::ptr::NonNull;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayFormat {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bits_per_pixel: u8,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayError {
    Absent,
    UnsupportedFormat,
    InvalidExtent,
    BufferTooSmall,
    Lost,
}

impl DisplayError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "display-absent",
            Self::UnsupportedFormat => "display-format-unsupported",
            Self::InvalidExtent => "display-extent-invalid",
            Self::BufferTooSmall => "display-buffer-too-small",
            Self::Lost => "display-base-lost",
        }
    }
}

impl DisplayFormat {
    pub fn validate(self) -> Result<Self, DisplayError> {
        let row_bytes = self
            .width
            .checked_mul(4)
            .ok_or(DisplayError::InvalidExtent)?;
        if self.width == 0 || self.height == 0 || self.pitch < row_bytes {
            return Err(DisplayError::InvalidExtent);
        }
        if self.bits_per_pixel != 32
            || [self.red_shift, self.green_shift, self.blue_shift]
                .into_iter()
                .any(|shift| shift > 24 || shift % 8 != 0)
            || self.red_shift == self.green_shift
            || self.red_shift == self.blue_shift
            || self.green_shift == self.blue_shift
        {
            return Err(DisplayError::UnsupportedFormat);
        }
        self.byte_len()?;
        Ok(self)
    }

    pub fn byte_len(self) -> Result<usize, DisplayError> {
        usize::try_from(
            u64::from(self.pitch)
                .checked_mul(u64::from(self.height))
                .ok_or(DisplayError::InvalidExtent)?,
        )
        .map_err(|_| DisplayError::InvalidExtent)
    }

    fn pixel(self, red: u8, green: u8, blue: u8) -> u32 {
        (u32::from(red) << self.red_shift)
            | (u32::from(green) << self.green_shift)
            | (u32::from(blue) << self.blue_shift)
    }
}

pub trait PixelTarget {
    fn format(&self) -> DisplayFormat;
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError>;
}

/// Retained readable pixels support coverage blending without assuming a
/// background color. Scanout-only Bases need not offer this capability.
pub trait RetainedPixelTarget: PixelTarget {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError>;
}

impl<T: RetainedPixelTarget + ?Sized> RetainedPixelTarget for &mut T {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        (**self).read_pixel(x, y)
    }
}

impl<T: PixelTarget + ?Sized> PixelTarget for &mut T {
    fn format(&self) -> DisplayFormat {
        (**self).format()
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        (**self).write_pixel(x, y, pixel)
    }
}

pub struct RawDisplay {
    address: NonNull<u8>,
    byte_len: usize,
    format: DisplayFormat,
    available: bool,
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
        })
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
        // SAFETY: construction establishes the writable range and the bound
        // above keeps this four-byte volatile write inside it.
        unsafe {
            let output = pixel.to_le_bytes();
            for (index, byte) in output.into_iter().enumerate() {
                core::ptr::write_volatile(self.address.as_ptr().add(offset + index), byte);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DisplayReceipt {
    pub commands: u8,
    pub pixels_written: u32,
}

pub fn render_scene(
    target: &mut impl PixelTarget,
    scene: &GraphicsScene,
) -> Result<DisplayReceipt, DisplayError> {
    let format = target.format().validate()?;
    let mut receipt = DisplayReceipt::default();
    for command in scene.commands() {
        receipt.commands = receipt
            .commands
            .checked_add(1)
            .ok_or(DisplayError::InvalidExtent)?;
        render_command(target, format, command, &mut receipt)?;
    }
    Ok(receipt)
}

/// Manifest one portable gray8 bitmap on the selected finite display Base.
/// Nearest-neighbor scaling is a mechanism choice below the bitmap and
/// presentation contracts; it does not mutate their semantic truth.
pub fn render_gray8_bitmap(
    target: &mut impl PixelTarget,
    bitmap: &conduit_presentation::Gray8Bitmap,
) -> Result<DisplayReceipt, DisplayError> {
    let format = target.format().validate()?;
    let source_width = u64::from(bitmap.width());
    let source_height = u64::from(bitmap.height());
    let mut receipt = DisplayReceipt {
        commands: 1,
        pixels_written: 0,
    };
    for y in 0..format.height {
        let source_y = u64::from(y) * source_height / u64::from(format.height);
        for x in 0..format.width {
            let source_x = u64::from(x) * source_width / u64::from(format.width);
            let index = usize::try_from(source_y * source_width + source_x)
                .map_err(|_| DisplayError::InvalidExtent)?;
            let gray = bitmap.pixels()[index];
            put(target, x, y, format.pixel(gray, gray, gray), &mut receipt)?;
        }
    }
    Ok(receipt)
}

fn render_command(
    target: &mut impl PixelTarget,
    format: DisplayFormat,
    command: &GraphicsCommand,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let Some(bounds) = clipped(command.bounds, command.clip, format) else {
        return Ok(());
    };
    let color = paint(format, command.paint);
    match command.kind {
        GraphicsCommandKind::OrthogonalPath => {
            let path = command.path_geometry().ok_or(DisplayError::InvalidExtent)?;
            for segment in path.points().windows(2) {
                let start = segment[0];
                let end = segment[1];
                let rect = LayoutRect {
                    x: start.x.min(end.x),
                    y: start.y.min(end.y),
                    width: (i32::from(start.x) - i32::from(end.x)).unsigned_abs() as u16 + 1,
                    height: (i32::from(start.y) - i32::from(end.y)).unsigned_abs() as u16 + 1,
                };
                if let Some(visible) = clipped(rect, command.clip, format) {
                    fill(target, visible, color, receipt)?;
                }
            }
            Ok(())
        }
        GraphicsCommandKind::Rect if command.style == GraphicsShapeStyle::Fill => {
            fill(target, bounds, color, receipt)
        }
        GraphicsCommandKind::Rect => stroke(target, bounds, color, receipt),
        GraphicsCommandKind::Text | GraphicsCommandKind::Icon => text(
            target,
            command.bounds,
            bounds,
            command.payload(),
            color,
            receipt,
        ),
    }
}

fn clipped(bounds: LayoutRect, clip: LayoutRect, format: DisplayFormat) -> Option<LayoutRect> {
    let left = i32::from(bounds.x).max(i32::from(clip.x)).max(0);
    let top = i32::from(bounds.y).max(i32::from(clip.y)).max(0);
    let right = (i32::from(bounds.x) + i32::from(bounds.width))
        .min(i32::from(clip.x) + i32::from(clip.width))
        .min(i32::try_from(format.width).ok()?);
    let bottom = (i32::from(bounds.y) + i32::from(bounds.height))
        .min(i32::from(clip.y) + i32::from(clip.height))
        .min(i32::try_from(format.height).ok()?);
    (right > left && bottom > top).then_some(LayoutRect {
        x: i16::try_from(left).ok()?,
        y: i16::try_from(top).ok()?,
        width: u16::try_from(right - left).ok()?,
        height: u16::try_from(bottom - top).ok()?,
    })
}

fn paint(format: DisplayFormat, role: GraphicsPaintRole) -> u32 {
    let (red, green, blue) = match role {
        GraphicsPaintRole::Background => BACKGROUND,
        GraphicsPaintRole::Foreground => FOREGROUND,
        GraphicsPaintRole::Accent => ACCENT,
        GraphicsPaintRole::Status | GraphicsPaintRole::Warning => WARNING,
        GraphicsPaintRole::Muted => MUTED,
        GraphicsPaintRole::Success => SUCCESS,
        GraphicsPaintRole::Danger => DANGER,
    };
    format.pixel(red, green, blue)
}

fn fill(
    target: &mut impl PixelTarget,
    rect: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    for y in u32::from(rect.y as u16)..u32::from(rect.y as u16) + u32::from(rect.height) {
        for x in u32::from(rect.x as u16)..u32::from(rect.x as u16) + u32::from(rect.width) {
            put(target, x, y, color, receipt)?;
        }
    }
    Ok(())
}

fn stroke(
    target: &mut impl PixelTarget,
    rect: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let left = u32::from(rect.x as u16);
    let top = u32::from(rect.y as u16);
    let right = left + u32::from(rect.width) - 1;
    let bottom = top + u32::from(rect.height) - 1;
    for x in left..=right {
        put(target, x, top, color, receipt)?;
        if bottom != top {
            put(target, x, bottom, color, receipt)?;
        }
    }
    for y in top.saturating_add(1)..bottom {
        put(target, left, y, color, receipt)?;
        if right != left {
            put(target, right, y, color, receipt)?;
        }
    }
    Ok(())
}

// Private bounded Unifont raster mechanism. Portable text meaning remains the
// exact UTF-8 payload above this boundary; unsupported glyphs use U+FFFD.
fn text(
    target: &mut impl PixelTarget,
    rect: LayoutRect,
    clip: LayoutRect,
    value: &str,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let mut cursor = text_layout::TextCursor::new(rect.width);
    for character in value.chars() {
        let Some((cell_x, cell_y)) = cursor.advance(character) else {
            continue;
        };
        let (glyph, _) = font::glyph(character);
        let width = u32::from(glyph.width);
        if cell_y >= u32::from(rect.height) || cell_x + width > u32::from(rect.width) {
            break;
        }
        for row in 0..font::GLYPH_HEIGHT {
            for column in 0..width {
                let x = i32::from(rect.x) + (cell_x + column) as i32;
                let y = i32::from(rect.y) + (cell_y + row) as i32;
                if x < i32::from(clip.x)
                    || y < i32::from(clip.y)
                    || x >= i32::from(clip.x) + i32::from(clip.width)
                    || y >= i32::from(clip.y) + i32::from(clip.height)
                {
                    continue;
                }
                let byte = glyph.bitmap[row as usize * (width as usize / 8) + column as usize / 8];
                if byte & (0x80 >> (column % 8)) != 0 {
                    put(target, x as u32, y as u32, color, receipt)?;
                }
            }
        }
    }
    Ok(())
}

fn put(
    target: &mut impl PixelTarget,
    x: u32,
    y: u32,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    target.write_pixel(x, y, color)?;
    receipt.pixels_written = receipt
        .pixels_written
        .checked_add(1)
        .ok_or(DisplayError::InvalidExtent)?;
    Ok(())
}

#[cfg(test)]
mod tests;
