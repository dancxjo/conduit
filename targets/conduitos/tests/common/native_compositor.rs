use conduitos::display::{DisplayError, DisplayFormat, PixelTarget};

#[derive(Clone)]
pub struct MemoryDisplay {
    pub format: DisplayFormat,
    pub pixels: Vec<u32>,
    pub lost: bool,
}

impl MemoryDisplay {
    pub fn new() -> Self {
        Self {
            format: DisplayFormat {
                width: 32,
                height: 16,
                pitch: 128,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            },
            pixels: vec![0; 32 * 16],
            lost: false,
        }
    }

    pub fn lost() -> Self {
        let mut display = Self::new();
        display.lost = true;
        display
    }
}

impl PixelTarget for MemoryDisplay {
    fn format(&self) -> DisplayFormat {
        self.format
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if self.lost {
            return Err(DisplayError::Lost);
        }
        let index = usize::try_from(y * self.format.width + x).unwrap();
        self.pixels[index] = pixel;
        Ok(())
    }
}
