//! Native retained-display discovery and boot diagnostics. Firmware remains
//! responsible for HDMI mode/clocks/power; only read selectors are changed.
//! Tegra210 controller/MC addresses follow upstream tegra210.dtsi. Selector,
//! cursor and window bits follow Linux drivers/gpu/drm/tegra/dc.h and dc.c.

use super::display::{Descriptor, PixelFormat, Refusal, Snapshot};
use super::memory::{Region, Regions};
use core::cell::RefCell;

pub const DC_BASES: [u64; 2] = [0x5420_0000, 0x5424_0000];
pub const MC_BASE: u64 = 0x7001_9000;

/// Root-owned, exclusive device-register access. DC offsets are word indices;
/// MC offsets are byte offsets. Accessors must use volatile, ordered device
/// mappings. `controller_accessible` must establish clock-enabled/out-of-reset
/// state before any DC read; a DT node's existence alone is insufficient.
pub trait Registers {
    fn controller_accessible(&mut self, controller: usize) -> bool;
    fn read_dc(&mut self, controller: usize, word: u32) -> u32;
    fn write_dc(&mut self, controller: usize, word: u32, value: u32);
    fn read_mc(&mut self, byte: u32) -> u32;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Unavailable,
    Selector,
    Overlay,
    Ambiguous,
    Scanout(Refusal),
    DiagnosticBounds,
}

struct Selectors<'a, R: Registers> {
    registers: &'a mut R,
    controller: usize,
    access: u32,
    window: u32,
}

impl<R: Registers> Drop for Selectors<'_, R> {
    fn drop(&mut self) {
        self.registers.write_dc(self.controller, 0x042, self.window);
        self.registers.write_dc(self.controller, 0x040, self.access);
        // Complete posted selector restoration before the transaction returns.
        let _ = self.registers.read_dc(self.controller, 0x040);
    }
}

fn controller<R: Registers>(
    registers: &mut R,
    index: usize,
    firmware: Region,
    protected: &Regions,
) -> Result<Descriptor, Error> {
    let access = registers.read_dc(index, 0x040);
    let window = registers.read_dc(index, 0x042);
    let transaction = Selectors {
        registers,
        controller: index,
        access,
        window,
    };
    transaction.registers.write_dc(index, 0x040, access | 1);
    if transaction.registers.read_dc(index, 0x040) != access | 1 {
        return Err(Error::Selector);
    }
    // A retained cursor could cover boot diagnostics. Refuse rather than
    // disable it or claim exclusive visible output while it remains enabled.
    if transaction.registers.read_dc(index, 0x402) & (1 << 16) != 0 {
        return Err(Error::Overlay);
    }
    let mut result = None;
    for selection in [0x10, 0x20, 0x40] {
        transaction.registers.write_dc(index, 0x042, selection);
        if transaction.registers.read_dc(index, 0x042) != selection {
            return Err(Error::Selector);
        }
        let shared = RefCell::new(&mut *transaction.registers);
        let snapshot = Snapshot::capture(
            index,
            |word| shared.borrow_mut().read_dc(index, word),
            |byte| shared.borrow_mut().read_mc(byte),
        )
        .map_err(Error::Scanout)?;
        if snapshot.options & (1 << 30) == 0 {
            continue;
        }
        if result.is_some() {
            return Err(Error::Overlay);
        }
        result = Some(
            snapshot
                .descriptor(firmware, protected)
                .map_err(Error::Scanout)?,
        );
    }
    let result = result.ok_or(Error::Unavailable);
    transaction.registers.write_dc(index, 0x042, window);
    transaction.registers.write_dc(index, 0x040, access);
    if transaction.registers.read_dc(index, 0x042) != window
        || transaction.registers.read_dc(index, 0x040) != access
    {
        return Err(Error::Selector);
    }
    result
}

/// Probe both safely accessible controllers, requiring one unambiguous match
/// to firmware's allocation. No modesetting, clock or SMMU writes occur.
/// Firmware/secondary cores must be quiescent throughout this transaction.
pub fn probe(
    registers: &mut impl Registers,
    firmware: Region,
    protected: &Regions,
) -> Result<Descriptor, Error> {
    let mut found = None;
    let mut refusal = Error::Unavailable;
    for index in 0..2 {
        if !registers.controller_accessible(index) {
            continue;
        }
        match controller(registers, index, firmware, protected) {
            Ok(descriptor) => {
                if found.is_some() {
                    return Err(Error::Ambiguous);
                }
                found = Some(descriptor);
            }
            Err(error) => refusal = error,
        }
    }
    found.ok_or(refusal)
}

/// Fixed 5x7 diagnostic font for early boot only; no input, retained UI state or
/// substitute Face interaction path. Unsupported characters use a question mark.
fn glyph(byte: u8) -> [u8; 7] {
    match byte.to_ascii_uppercase() {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b' ' => [0; 7],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b':' => [0, 12, 12, 0, 12, 12, 0],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'=' => [0, 31, 0, 31, 0, 0, 0],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}

/// Render at most 512 ASCII diagnostic bytes, wrapping within a 2x font grid.
/// Caller writes only this descriptor's CPU-accessible framebuffer; offsets
/// passed to `pixel` are bytes relative to its allocation. Colors include an
/// opaque alpha byte. This routine allocates nothing and never touches padding.
pub fn diagnostic(
    descriptor: Descriptor,
    message: &str,
    mut pixel: impl FnMut(usize, u32),
) -> Result<(), Error> {
    if message.len() > 512 || !message.is_ascii() || descriptor.width < 12 || descriptor.height < 16
    {
        return Err(Error::DiagnosticBounds);
    }
    let columns = descriptor.width / 12;
    let rows = descriptor.height / 16;
    let mut column = 0;
    let mut row = 0;
    // Preflight the whole message before any effect.
    for byte in message.bytes() {
        if byte == b'\n' {
            row += 1;
            column = 0;
        } else {
            if column == columns {
                row += 1;
                column = 0;
            }
            column += 1;
        }
        if row >= rows {
            return Err(Error::DiagnosticBounds);
        }
    }
    let foreground = match descriptor.format {
        PixelFormat::Bgra8888 => 0xffc0_ffee,
        PixelFormat::Rgba8888 => 0xffee_ffc0,
    };
    column = 0;
    row = 0;
    for byte in message.bytes() {
        if byte == b'\n' {
            row += 1;
            column = 0;
            continue;
        }
        if column == columns {
            row += 1;
            column = 0;
        }
        let shape = glyph(byte);
        for y in 0..16 {
            for x in 0..12 {
                let ink = y < 14 && x < 10 && shape[(y / 2) as usize] & (1 << (4 - x / 2)) != 0;
                let offset = u64::from(row * 16 + y) * u64::from(descriptor.stride)
                    + u64::from(column * 12 + x) * 4;
                if offset + 4 > descriptor.memory.bytes() {
                    return Err(Error::DiagnosticBounds);
                }
                pixel(offset as usize, if ink { foreground } else { 0xff08_0808 });
            }
        }
        column += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Device {
        access: [u32; 2],
        selection: [u32; 2],
        accessible: [bool; 2],
        extra_window: bool,
        cursor: bool,
        writes: usize,
    }
    impl Device {
        fn new() -> Self {
            Self {
                access: [0; 2],
                selection: [0x40; 2],
                accessible: [true, false],
                extra_window: false,
                cursor: false,
                writes: 0,
            }
        }
    }
    impl Registers for Device {
        fn controller_accessible(&mut self, controller: usize) -> bool {
            self.accessible[controller]
        }
        fn read_dc(&mut self, controller: usize, word: u32) -> u32 {
            assert!(
                self.accessible[controller],
                "must not read a gated controller"
            );
            match word {
                0x040 => self.access[controller],
                0x042 => self.selection[controller],
                0x032 => 1 << 5,
                0x402 => {
                    if self.cursor {
                        1 << 16
                    } else {
                        0
                    }
                }
                0x409 | 0x705 => 1080 << 16 | 1920,
                0x706 => 1080 << 16 | 7680,
                0x700 => {
                    if self.selection[controller] == 0x10 || self.extra_window {
                        1 << 30
                    } else {
                        0
                    }
                }
                0x703 => 12,
                0x709 => 0x1000_1000,
                0x70a => 7680,
                0x800 => 0x9200_0000,
                _ => 0,
            }
        }
        fn write_dc(&mut self, controller: usize, word: u32, value: u32) {
            self.writes += 1;
            match word {
                0x040 => self.access[controller] = value,
                0x042 => self.selection[controller] = value,
                _ => panic!("non-selector display write"),
            }
        }
        fn read_mc(&mut self, byte: u32) -> u32 {
            assert!(matches!(byte, 0x10 | 0x228 | 0x240 | 0x244));
            0
        }
    }
    fn firmware() -> Region {
        Region::new(0x9200_0000, 8 * 1024 * 1024).unwrap()
    }

    #[test]
    fn selector_transaction_restores_firmware_on_success_and_refusal() {
        let mut device = Device::new();
        assert_eq!(
            probe(&mut device, firmware(), &Regions::new())
                .unwrap()
                .width,
            1920
        );
        assert_eq!(device.access, [0; 2]);
        assert_eq!(device.selection, [0x40; 2]);
        assert!(device.writes < 16);
        device.extra_window = true;
        assert_eq!(
            probe(&mut device, firmware(), &Regions::new()),
            Err(Error::Overlay)
        );
        assert_eq!(device.access, [0; 2]);
        assert_eq!(device.selection, [0x40; 2]);
        device.cursor = true;
        assert_eq!(
            probe(&mut device, firmware(), &Regions::new()),
            Err(Error::Overlay)
        );
        assert_eq!(device.access, [0; 2]);
        assert_eq!(device.selection, [0x40; 2]);
    }

    #[test]
    fn both_controllers_cannot_claim_one_retained_allocation() {
        let mut device = Device::new();
        device.accessible[1] = true;
        assert_eq!(
            probe(&mut device, firmware(), &Regions::new()),
            Err(Error::Ambiguous)
        );
        assert_eq!(device.access, [0; 2]);
        assert_eq!(device.selection, [0x40; 2]);
    }

    #[test]
    fn diagnostics_respect_pitch_extent_and_preflight_before_effect() {
        let descriptor = probe(&mut Device::new(), firmware(), &Regions::new()).unwrap();
        let mut count = 0;
        let mut ink = 0;
        diagnostic(descriptor, "ConduitOS\nNANO BOOT", |offset, pixel| {
            assert!(offset as u64 + 4 <= descriptor.memory.bytes());
            assert!(offset % (descriptor.stride as usize) < descriptor.width as usize * 4);
            count += 1;
            if pixel != 0xff08_0808 {
                ink += 1;
            }
        })
        .unwrap();
        assert_eq!(count, 18 * 12 * 16);
        assert!(ink > 0);
        let tiny = Descriptor {
            width: 12,
            height: 16,
            ..descriptor
        };
        assert_eq!(
            diagnostic(tiny, "TOO LONG", |_, _| panic!("partial write")),
            Err(Error::DiagnosticBounds)
        );
        assert_eq!(
            diagnostic(descriptor, "é", |_, _| panic!("partial write")),
            Err(Error::DiagnosticBounds)
        );
    }
}
