//! Finite framebuffer mechanism below portable graphics meaning.

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
        GraphicsCommandKind::Rect if command.style == GraphicsShapeStyle::Fill => {
            fill(target, bounds, color, receipt)
        }
        GraphicsCommandKind::Rect => stroke(target, bounds, color, receipt),
        GraphicsCommandKind::Text | GraphicsCommandKind::Icon => {
            text(target, bounds, command.payload(), color, receipt)
        }
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
        GraphicsPaintRole::Background => (8, 18, 24),
        GraphicsPaintRole::Foreground => (205, 235, 224),
        GraphicsPaintRole::Accent => (69, 255, 188),
        GraphicsPaintRole::Status => (255, 190, 70),
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

// Private bounded 5x7 raster mechanism. Portable text meaning remains the
// exact UTF-8 payload above this boundary; unsupported glyphs use one box.
fn text(
    target: &mut impl PixelTarget,
    rect: LayoutRect,
    value: &str,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let origin_x = u32::from(rect.x as u16);
    let origin_y = u32::from(rect.y as u16);
    let right = origin_x + u32::from(rect.width);
    let bottom = origin_y + u32::from(rect.height);
    for (index, character) in value.chars().enumerate() {
        let cell_x = origin_x + u32::try_from(index).unwrap_or(u32::MAX).saturating_mul(6);
        if cell_x + 5 > right {
            break;
        }
        let rows = glyph(character);
        for row in 0..7_u32 {
            if origin_y + row >= bottom {
                break;
            }
            for column in 0..5_u32 {
                if rows[row as usize] & (0x10 >> column) != 0 {
                    put(target, cell_x + column, origin_y + row, color, receipt)?;
                }
            }
        }
    }
    Ok(())
}

fn glyph(character: char) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        _ => [31, 17, 21, 21, 21, 17, 31],
    }
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
mod tests {
    use super::*;
    use conduit_presentation::{GraphicsCommand, GraphicsShapeStyle};

    struct Buffer<'a> {
        format: DisplayFormat,
        bytes: &'a mut [u8],
        lost: bool,
    }

    impl PixelTarget for Buffer<'_> {
        fn format(&self) -> DisplayFormat {
            self.format
        }

        fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
            if self.lost {
                return Err(DisplayError::Lost);
            }
            let offset =
                usize::try_from(u64::from(y) * u64::from(self.format.pitch) + u64::from(x) * 4)
                    .map_err(|_| DisplayError::BufferTooSmall)?;
            let output = self
                .bytes
                .get_mut(offset..offset + 4)
                .ok_or(DisplayError::BufferTooSmall)?;
            output.copy_from_slice(&pixel.to_le_bytes());
            Ok(())
        }
    }

    fn format() -> DisplayFormat {
        DisplayFormat {
            width: 32,
            height: 24,
            pitch: 128,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }

    #[test]
    fn bounded_scene_renders_and_loss_remains_distinct() {
        let bounds = LayoutRect {
            x: 2,
            y: 2,
            width: 20,
            height: 12,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::rect(
                    bounds,
                    bounds,
                    GraphicsPaintRole::Accent,
                    GraphicsShapeStyle::Stroke,
                )
                .unwrap(),
            )
            .unwrap();
        scene
            .push(
                GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, "OK").unwrap(),
            )
            .unwrap();
        let mut bytes = [0_u8; 32 * 24 * 4];
        let mut target = Buffer {
            format: format(),
            bytes: &mut bytes,
            lost: false,
        };
        let receipt = render_scene(&mut target, &scene).unwrap();
        assert_eq!(receipt.commands, 2);
        assert!(receipt.pixels_written > 0);
        assert!(bytes.iter().any(|byte| *byte != 0));
        let first_glyph_pixel = 2 * 128 + 3 * 4;
        assert_eq!(
            &bytes[first_glyph_pixel..first_glyph_pixel + 4],
            &format().pixel(205, 235, 224).to_le_bytes()
        );

        let mut lost = Buffer {
            format: format(),
            bytes: &mut bytes,
            lost: true,
        };
        assert_eq!(render_scene(&mut lost, &scene), Err(DisplayError::Lost));
    }

    #[test]
    fn unsupported_format_and_small_buffer_refuse() {
        let mut invalid = format();
        invalid.bits_per_pixel = 24;
        assert_eq!(invalid.validate(), Err(DisplayError::UnsupportedFormat));

        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::rect(
                    bounds,
                    bounds,
                    GraphicsPaintRole::Background,
                    GraphicsShapeStyle::Fill,
                )
                .unwrap(),
            )
            .unwrap();
        let mut bytes = [0_u8; 4];
        let mut target = Buffer {
            format: format(),
            bytes: &mut bytes,
            lost: false,
        };
        assert_eq!(
            render_scene(&mut target, &scene),
            Err(DisplayError::BufferTooSmall)
        );
    }

    #[test]
    fn portable_gray8_bitmap_reaches_exact_framebuffer_pixels() {
        let bitmap =
            conduit_presentation::Gray8Bitmap::new(2, 2, alloc::vec![0, 64, 128, 255]).unwrap();
        let mut exact = format();
        exact.width = 4;
        exact.height = 4;
        exact.pitch = 16;
        let mut bytes = [0_u8; 4 * 4 * 4];
        {
            let mut target = Buffer {
                format: exact,
                bytes: &mut bytes,
                lost: false,
            };
            assert_eq!(
                render_gray8_bitmap(&mut target, &bitmap),
                Ok(DisplayReceipt {
                    commands: 1,
                    pixels_written: 16,
                })
            );
            target.lost = true;
            assert_eq!(
                render_gray8_bitmap(&mut target, &bitmap),
                Err(DisplayError::Lost)
            );
        }
        let pixels = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u32::from_le_bytes(*pixel))
            .collect::<alloc::vec::Vec<_>>();
        assert_eq!(
            pixels,
            alloc::vec![
                0x0000_0000,
                0x0000_0000,
                0x0040_4040,
                0x0040_4040,
                0x0000_0000,
                0x0000_0000,
                0x0040_4040,
                0x0040_4040,
                0x0080_8080,
                0x0080_8080,
                0x00ff_ffff,
                0x00ff_ffff,
                0x0080_8080,
                0x0080_8080,
                0x00ff_ffff,
                0x00ff_ffff,
            ]
        );
    }
}
