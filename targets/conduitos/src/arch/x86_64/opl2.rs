//! Exact YM3812-compatible I/O mechanism beneath portable music semantics.

use crate::machine::{BaseError, Opl2Base, Opl2Pitch};

use super::io::{inb, outb};

const ADDRESS_PORT: u16 = 0x388;
const DATA_PORT: u16 = 0x389;
const CLOCK_HZ: u64 = 3_579_545;
const RESET_FIRST_REGISTER: u8 = 0x01;
const RESET_LAST_REGISTER: u8 = 0xf5;
const MAXIMUM_RESET_WRITES: u16 = 245;
const MAXIMUM_QUANTIZATION_PPM: u64 = 2_500;
const OPERATOR_OFFSETS: [(u8, u8); 9] = [
    (0x00, 0x03),
    (0x01, 0x04),
    (0x02, 0x05),
    (0x08, 0x0b),
    (0x09, 0x0c),
    (0x0a, 0x0d),
    (0x10, 0x13),
    (0x11, 0x14),
    (0x12, 0x15),
];

trait RegisterIo {
    fn write(&mut self, register: u8, value: u8);
}

pub struct Opl2;

impl Opl2 {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Opl2 {
    fn default() -> Self {
        Self::new()
    }
}

impl RegisterIo for Opl2 {
    fn write(&mut self, register: u8, value: u8) {
        // YM3812 requires >=3.3 us after the address and >=23 us after data.
        // Port reads provide the documented ISA delay without borrowing a
        // scheduler clock or hiding retry/work policy in this Base.
        unsafe {
            outb(ADDRESS_PORT, register);
            for _ in 0..6 {
                let _ = inb(ADDRESS_PORT);
            }
            outb(DATA_PORT, value);
            for _ in 0..35 {
                let _ = inb(ADDRESS_PORT);
            }
        }
    }
}

impl Opl2Base for Opl2 {
    fn reset(&mut self) -> Result<u16, BaseError> {
        reset(self)
    }

    fn configure_fixed_patch(&mut self, channel: u8) -> Result<u16, BaseError> {
        configure_fixed_patch(self, channel)
    }

    fn key_on(&mut self, channel: u8, pitch_millihertz: u64) -> Result<Opl2Pitch, BaseError> {
        key_on(self, channel, pitch_millihertz)
    }

    fn key_off(&mut self, channel: u8) -> Result<(), BaseError> {
        key_off(self, channel)
    }

    fn quiesce(&mut self) -> Result<u16, BaseError> {
        quiesce(self)
    }
}

fn reset(io: &mut impl RegisterIo) -> Result<u16, BaseError> {
    let mut writes = 0u16;
    for register in RESET_FIRST_REGISTER..=RESET_LAST_REGISTER {
        io.write(register, 0);
        writes = writes.checked_add(1).ok_or(BaseError::WorkPressure)?;
    }
    if writes > MAXIMUM_RESET_WRITES {
        return Err(BaseError::WorkPressure);
    }
    Ok(writes)
}

fn configure_fixed_patch(io: &mut impl RegisterIo, channel: u8) -> Result<u16, BaseError> {
    let (modulator, carrier) = offsets(channel)?;
    let writes = [
        (0x20 + modulator, 0x01),
        (0x20 + carrier, 0x01),
        (0x40 + modulator, 0x18),
        (0x40 + carrier, 0x00),
        (0x60 + modulator, 0xf2),
        (0x60 + carrier, 0xf2),
        (0x80 + modulator, 0x74),
        (0x80 + carrier, 0x74),
        (0xe0 + modulator, 0x00),
        (0xe0 + carrier, 0x00),
        (0xc0 + channel, 0x00),
    ];
    for (register, value) in writes {
        io.write(register, value);
    }
    Ok(writes.len() as u16)
}

fn key_on(
    io: &mut impl RegisterIo,
    channel: u8,
    pitch_millihertz: u64,
) -> Result<Opl2Pitch, BaseError> {
    offsets(channel)?;
    let pitch = quantize(pitch_millihertz)?;
    io.write(0xa0 + channel, pitch.f_number as u8);
    io.write(
        0xb0 + channel,
        0x20 | (pitch.block << 2) | ((pitch.f_number >> 8) as u8 & 0x03),
    );
    Ok(pitch)
}

fn key_off(io: &mut impl RegisterIo, channel: u8) -> Result<(), BaseError> {
    offsets(channel)?;
    io.write(0xb0 + channel, 0);
    Ok(())
}

fn quiesce(io: &mut impl RegisterIo) -> Result<u16, BaseError> {
    for channel in 0..9 {
        key_off(io, channel)?;
    }
    Ok(9)
}

fn offsets(channel: u8) -> Result<(u8, u8), BaseError> {
    OPERATOR_OFFSETS
        .get(usize::from(channel))
        .copied()
        .ok_or(BaseError::OutOfRange)
}

fn quantize(requested_millihertz: u64) -> Result<Opl2Pitch, BaseError> {
    for block in 0..=7u8 {
        let numerator = u128::from(requested_millihertz) * 72u128 * (1u128 << (20 - block));
        let denominator = u128::from(CLOCK_HZ) * 1_000;
        let f_number = ((numerator + denominator / 2) / denominator) as u64;
        if !(256..=1_023).contains(&f_number) {
            continue;
        }
        let realized = f_number * CLOCK_HZ * 1_000 / 72 / (1u64 << (20 - block));
        let error = realized.abs_diff(requested_millihertz);
        if error.saturating_mul(1_000_000)
            > requested_millihertz.saturating_mul(MAXIMUM_QUANTIZATION_PPM)
        {
            return Err(BaseError::OutOfRange);
        }
        return Ok(Opl2Pitch {
            requested_millihertz,
            realized_millihertz: realized,
            f_number: f_number as u16,
            block,
        });
    }
    Err(BaseError::OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[derive(Default)]
    struct Recorder(Vec<(u8, u8)>);

    impl RegisterIo for Recorder {
        fn write(&mut self, register: u8, value: u8) {
            self.0.push((register, value));
        }
    }

    #[test]
    fn fixed_patch_and_chord_use_distinct_native_channels_then_quiesce() {
        let mut io = Recorder::default();
        assert_eq!(configure_fixed_patch(&mut io, 0), Ok(11));
        assert_eq!(configure_fixed_patch(&mut io, 1), Ok(11));
        assert_eq!(configure_fixed_patch(&mut io, 2), Ok(11));
        let a = key_on(&mut io, 0, 440_000).unwrap();
        let c = key_on(&mut io, 1, 523_251).unwrap();
        let e = key_on(&mut io, 2, 659_255).unwrap();
        assert!(a.realized_millihertz.abs_diff(440_000) < 1_100);
        assert!(c.realized_millihertz.abs_diff(523_251) < 1_400);
        assert!(e.realized_millihertz.abs_diff(659_255) < 1_700);
        assert_eq!(quiesce(&mut io), Ok(9));
        assert!(io.0.ends_with(&[
            (0xb0, 0),
            (0xb1, 0),
            (0xb2, 0),
            (0xb3, 0),
            (0xb4, 0),
            (0xb5, 0),
            (0xb6, 0),
            (0xb7, 0),
            (0xb8, 0),
        ]));
    }

    #[test]
    fn invalid_channel_and_unrepresentable_pitch_refuse() {
        let mut io = Recorder::default();
        assert_eq!(key_on(&mut io, 9, 440_000), Err(BaseError::OutOfRange));
        assert_eq!(key_on(&mut io, 0, 1), Err(BaseError::OutOfRange));
        assert!(io.0.is_empty());
    }

    #[test]
    fn reset_has_one_exact_finite_write_budget() {
        let mut io = Recorder::default();
        assert_eq!(reset(&mut io), Ok(245));
        assert_eq!(io.0.first(), Some(&(0x01, 0)));
        assert_eq!(io.0.last(), Some(&(0xf5, 0)));
    }
}
