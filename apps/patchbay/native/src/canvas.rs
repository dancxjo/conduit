//! Bounded `embedded-graphics` adaptation for the softbuffer pixel slice.

use core::convert::Infallible;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::pixelcolor::{Rgb888, RgbColor};
use embedded_graphics::prelude::{DrawTarget, Pixel, Point};

pub struct SoftwareCanvas<'a> {
    pixels: &'a mut [u32],
    width: usize,
    height: usize,
}

impl<'a> SoftwareCanvas<'a> {
    pub fn new(pixels: &'a mut [u32], width: usize, height: usize) -> Self {
        Self {
            pixels,
            width,
            height,
        }
    }
}

impl OriginDimensions for SoftwareCanvas<'_> {
    fn size(&self) -> Size {
        Size::new(
            self.width.min(u32::MAX as usize) as u32,
            self.height.min(u32::MAX as usize) as u32,
        )
    }
}

impl DrawTarget for SoftwareCanvas<'_> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(Point { x, y }, color) in pixels {
            let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
                continue;
            };
            if x >= self.width || y >= self.height {
                continue;
            }
            if let Some(index) = y
                .checked_mul(self.width)
                .and_then(|row| row.checked_add(x))
                .filter(|index| *index < self.pixels.len())
            {
                self.pixels[index] = rgb888_to_softbuffer(color);
            }
        }
        Ok(())
    }
}

pub const fn softbuffer_to_rgb888(color: u32) -> Rgb888 {
    Rgb888::new(
        ((color >> 16) & 0xff) as u8,
        ((color >> 8) & 0xff) as u8,
        (color & 0xff) as u8,
    )
}

fn rgb888_to_softbuffer(color: Rgb888) -> u32 {
    ((color.r() as u32) << 16) | ((color.g() as u32) << 8) | color.b() as u32
}
