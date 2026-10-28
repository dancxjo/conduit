#![no_std]
//! Finite HT16K33 commands and explicit segment wiring. No clock, environmental
//! formatting, peripheral initialization, Body membership, or effect authority.
//! Caller owns the single I2C bus and bounds each operation with its deadline.
use embedded_hal_async::i2c::I2c;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Address(u8);
impl Address {
    pub const fn new(value: u8) -> Result<Self, Refusal> {
        if value >= 0x70 && value <= 0x77 {
            Ok(Self(value))
        } else {
            Err(Refusal::Address)
        }
    }
    pub const fn get(self) -> u8 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brightness(u8);
impl Brightness {
    pub const fn new(value: u8) -> Result<Self, Refusal> {
        if value <= 15 {
            Ok(Self(value))
        } else {
            Err(Refusal::Brightness)
        }
    }
    pub const fn get(self) -> u8 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Blink {
    Off = 0,
    Hz2 = 1,
    Hz1 = 2,
    HzHalf = 3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configuration {
    pub brightness: Brightness,
    pub blink: Blink,
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Address,
    Brightness,
    Wiring,
    Glyph,
}
/// Clear all RAM while display is disabled; enable only after every ACK.
/// A partial/cancelled initialization is not success; retry initialization.
pub async fn initialize<I: I2c>(
    bus: &mut I,
    address: Address,
    configuration: Configuration,
) -> Result<(), I::Error> {
    bus.write(address.0, &[0x21]).await?;
    bus.write(address.0, &[0x80]).await?;
    write_frame(bus, address, &[0; 16]).await?;
    set_brightness(bus, address, configuration.brightness).await?;
    set_display(bus, address, configuration.blink, configuration.enabled).await
}
pub async fn set_brightness<I: I2c>(
    bus: &mut I,
    address: Address,
    brightness: Brightness,
) -> Result<(), I::Error> {
    bus.write(address.0, &[0xe0 | brightness.0]).await
}
pub async fn set_display<I: I2c>(
    bus: &mut I,
    address: Address,
    blink: Blink,
    enabled: bool,
) -> Result<(), I::Error> {
    bus.write(
        address.0,
        &[0x80 | ((blink as u8) << 1) | u8::from(enabled)],
    )
    .await
}
/// One pointer+16-byte I2C transaction; successful return means bus ACK only,
/// not optical verification. A bus failure may leave partially updated RAM.
pub async fn write_frame<I: I2c>(
    bus: &mut I,
    address: Address,
    ram: &[u8; 16],
) -> Result<(), I::Error> {
    let mut packet = [0; 17];
    packet[1..].copy_from_slice(ram);
    bus.write(address.0, &packet).await
}

/// Explicit finite glyph subset, using Adafruit alpha14 logical segment order.
/// Font patterns adapted from Adafruit_LED_Backpack (MIT, LICENSE-ADAFRUIT).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph(u16);
impl Glyph {
    pub fn from_char(value: char) -> Result<Self, Refusal> {
        let bits = match value {
            ' ' => 0,
            '-' => 0x00c0,
            '0' => 0x0c3f,
            '1' => 0x0006,
            '2' => 0x00db,
            '3' => 0x008f,
            '4' => 0x00e6,
            '5' => 0x2069,
            '6' => 0x00fd,
            '7' => 0x0007,
            '8' => 0x00ff,
            '9' => 0x00ef,
            'A' => 0x00f7,
            'B' => 0x128f,
            'C' => 0x0039,
            'D' => 0x120f,
            'E' => 0x00f9,
            'F' => 0x0071,
            'G' => 0x00bd,
            'H' => 0x00f6,
            'I' => 0x1209,
            'J' => 0x001e,
            'K' => 0x2470,
            'L' => 0x0038,
            'M' => 0x0536,
            'N' => 0x2136,
            'O' => 0x003f,
            'P' => 0x00f3,
            'Q' => 0x203f,
            'R' => 0x20f3,
            'S' => 0x00ed,
            'T' => 0x1201,
            'U' => 0x003e,
            'V' => 0x0c30,
            'W' => 0x2836,
            'X' => 0x2d00,
            'Y' => 0x1500,
            'Z' => 0x0c09,
            'h' => 0x1070,
            'r' => 0x0050,
            'o' => 0x00dc,
            // Upper loop spans both middle halves on an alpha14 display.
            '°' => 0x00e3,
            _ => return Err(Refusal::Glyph),
        };
        Ok(Self(bits))
    }
}
/// Four positions map to distinct 16-bit RAM words; logical segment bits map
/// to distinct physical bits. DP is separate and may not alias a segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    words: [u8; 4],
    segment_bits: [u8; 14],
    decimal_bit: u8,
}
impl Profile {
    pub fn new(words: [u8; 4], segment_bits: [u8; 14], decimal_bit: u8) -> Result<Self, Refusal> {
        if decimal_bit >= 16 {
            return Err(Refusal::Wiring);
        }
        let mut seen = 1u16 << decimal_bit;
        for bit in segment_bits {
            if bit >= 16 || seen & (1u16 << bit) != 0 {
                return Err(Refusal::Wiring);
            }
            seen |= 1u16 << bit;
        }
        let mut seen = 0u8;
        for word in words {
            if word >= 8 || seen & (1u8 << word) != 0 {
                return Err(Refusal::Wiring);
            }
            seen |= 1u8 << word;
        }
        Ok(Self {
            words,
            segment_bits,
            decimal_bit,
        })
    }
    /// Common alpha14 backpack mapping, selected explicitly by Host profile;
    /// the controller model alone does not establish a backpack's wiring.
    pub fn adafruit_alpha4() -> Self {
        Self::new(
            [0, 1, 2, 3],
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13],
            14,
        )
        .expect("constant wiring")
    }
    pub fn encode(&self, glyphs: [Glyph; 4], decimal_points: [bool; 4]) -> [u8; 16] {
        let mut ram = [0; 16];
        for (index, glyph) in glyphs.iter().enumerate() {
            let mut physical = 0u16;
            for (logical, bit) in self.segment_bits.iter().enumerate() {
                if glyph.0 & (1 << logical) != 0 {
                    physical |= 1 << bit;
                }
            }
            if decimal_points[index] {
                physical |= 1 << self.decimal_bit;
            }
            let at = usize::from(self.words[index]) * 2;
            ram[at..at + 2].copy_from_slice(&physical.to_le_bytes());
        }
        ram
    }
}
