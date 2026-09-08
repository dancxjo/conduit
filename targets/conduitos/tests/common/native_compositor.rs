use conduitos::display::{DisplayError, DisplayFormat, PixelTarget};

#[derive(Clone)]
pub struct MemoryDisplay {
    pub format: DisplayFormat,
    pub pixels: Vec<u32>,
    pub lost: bool,
    pub writes: u32,
    pub fail_after: Option<u32>,
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
            writes: 0,
            fail_after: None,
        }
    }

    #[allow(dead_code)]
    pub fn lost() -> Self {
        let mut display = Self::new();
        display.lost = true;
        display
    }

    #[allow(dead_code)]
    pub fn reset_writes(&mut self) {
        self.writes = 0;
    }
}

impl PixelTarget for MemoryDisplay {
    fn format(&self) -> DisplayFormat {
        self.format
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if self.lost || self.fail_after == Some(self.writes) {
            return Err(DisplayError::Lost);
        }
        let index = usize::try_from(y * self.format.width + x).unwrap();
        self.pixels[index] = pixel;
        self.writes += 1;
        Ok(())
    }
}
