//! One firmware-allocated, uncached BCM2835 scanout beneath PixelTarget.
use super::mailbox;
use crate::display::{DisplayError, DisplayFormat, PixelTarget, RawDisplay};
use core::{
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};

static CLAIMED: AtomicBool = AtomicBool::new(false);
#[path = "framebuffer_response.rs"]
mod response;
#[path = "framebuffer_timing.rs"]
mod timing;

#[repr(C, align(16))]
struct Message<const WORDS: usize>([u32; WORDS]);

pub struct FirmwareScanout {
    raw: RawDisplay,
    physical: u32,
    bytes: u32,
    firmware_mode: (u32, u32),
    preferred_mode: Option<(u32, u32)>,
    preferred_clock_khz: Option<u32>,
    timing_set: Option<Result<(), &'static str>>,
}

impl FirmwareScanout {
    pub fn prepare() -> Result<Self, &'static str> {
        if CLAIMED.swap(true, Ordering::AcqRel) {
            return Err("bcm2835-framebuffer-already-owned");
        }
        let mut dimensions = Message([32, 0, 0x40003, 8, 0, 0, 0, 0]);
        mailbox::call(&mut dimensions.0)?;
        if dimensions.0[4] != 0x8000_0008 {
            return Err("bcm2835-display-dimensions-invalid");
        }
        let firmware_mode = (dimensions.0[5], dimensions.0[6]);
        let preferred = read_preferred_mode();
        let preferred_mode = preferred.map(|mode| (mode.width, mode.height));
        let (width, height) = preferred_mode.unwrap_or(firmware_mode);
        let pixels = u64::from(width) * u64::from(height);
        if width == 0
            || height == 0
            || pixels > crate::native_compositor::MAX_COMPOSITOR_PIXELS as u64
        {
            return Err("bcm2835-display-mode-outside-budget");
        }
        let timing_set = preferred.map(timing::set_hdmi_mode);
        // Digital scanout is a complete opaque canvas with no TV overscan.
        let mut canvas = Message([56, 0, 0x4800a, 16, 16, 0, 0, 0, 0, 0x48007, 4, 4, 2, 0]);
        mailbox::call(&mut canvas.0)?;
        if canvas.0[4] != 0x8000_0010
            || canvas.0[5..9] != [0; 4]
            || canvas.0[11] != 0x8000_0004
            || canvas.0[12] != 2
        {
            return Err("bcm2835-opaque-canvas-refused");
        }
        // Set and allocate together; firmware may not accept split requests.
        let mut message = Message([
            160, 0, 0x48003, 8, 8, width, height, 0x48004, 8, 8, width, height, 0x48009, 8, 8, 0,
            0, 0x48005, 4, 4, 32, 0x48006, 4, 4, 1, 0x40001, 8, 4, 4096, 0, 0x40008, 4, 0, 0,
            0x10006, 8, 0, 0, 0, 0,
        ]);
        mailbox::call(&mut message.0)?;
        let (physical, bytes, format) = response::validate(&message.0, width, height)?;
        let address =
            NonNull::new(physical as *mut u8).ok_or("bcm2835-framebuffer-address-invalid")?;
        // Firmware grants this sole Root the reserved buffer; identity-mapped
        // Normal NC GPU RAM and disabled data caching expose stores to scanout.
        let raw = unsafe { RawDisplay::new_word_writable(address, bytes as usize, format) }
            .map_err(|error| error.as_str())?;
        Ok(Self {
            raw,
            physical,
            bytes,
            firmware_mode,
            preferred_mode,
            preferred_clock_khz: preferred.map(|mode| mode.pixel_clock_khz),
            timing_set,
        })
    }
    /// Bounded Root observation of the sole granted scanout after rendering.
    pub fn observed_pixels(&self) -> [u32; 9] {
        let format = self.raw.format();
        let mut pixels = [0; 9];
        for (row, y) in [0, format.height / 2, format.height - 1]
            .into_iter()
            .enumerate()
        {
            for (column, x) in [0, format.width / 2, format.width - 1]
                .into_iter()
                .enumerate()
            {
                let offset = y as usize * format.pitch as usize + x as usize * 4;
                // Validation grants the full stride and all chosen coordinates.
                pixels[row * 3 + column] = unsafe {
                    core::ptr::read_volatile((self.physical as usize + offset) as *const u32)
                };
            }
        }
        pixels
    }
    pub fn hdmi_timing(&self) -> Option<(u32, u32, u32)> {
        let mut message = Message([0_u32; 15]);
        message.0[..6].copy_from_slice(&[60, 0, 0x40017, 36, 4, 2]); // HDMI0 display ID.
        mailbox::call(&mut message.0).ok()?;
        if message.0[4] != 0x8000_0024 || message.0[6] == 0 {
            return None;
        }
        Some((message.0[7] & 0xffff, message.0[9] >> 16, message.0[6]))
    }
    pub fn timing_set_observation(&self) -> (Option<u32>, Option<Result<(), &'static str>>) {
        (self.preferred_clock_khz, self.timing_set)
    }
    pub fn mode_observation(&self) -> ((u32, u32), Option<(u32, u32)>) {
        (self.firmware_mode, self.preferred_mode)
    }
    pub fn physical_range(&self) -> (u32, u32) {
        (self.physical, self.bytes)
    }
}
impl PixelTarget for FirmwareScanout {
    fn format(&self) -> DisplayFormat {
        self.raw.format()
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        self.raw.write_pixel(x, y, pixel)
    }
    fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
        self.raw.write_span(x, y, pixels)
    }
    fn commit_frame(&mut self) -> Result<(), DisplayError> {
        unsafe {
            core::arch::asm!("mcr p15, 0, {0}, c7, c10, 4",in(reg) 0_u32,options(nostack));
        }
        Ok(())
    }
}

fn read_preferred_mode() -> Option<crate::display::edid::PreferredTiming> {
    let mut message = Message([0_u32; 40]);
    message.0[..7].copy_from_slice(&[160, 0, 0x30020, 136, 4, 0, 0]);
    mailbox::call(&mut message.0).ok()?;
    if message.0[4] != 0x8000_0088 || message.0[5] != 0 || message.0[6] != 0 {
        return None;
    }
    let mut block = [0_u8; 128];
    for (chunk, word) in block.chunks_exact_mut(4).zip(&message.0[7..39]) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    crate::display::edid::preferred_timing(&block)
}
