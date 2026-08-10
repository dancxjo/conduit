use crate::machine::BaseError;

use super::io::{inb, outb};

const COM1: u16 = 0x3f8;
const SERIAL_SPIN_LIMIT: u32 = 100_000;
const MAX_PRESENT_BYTES: usize = 16;

pub(super) fn initialize() {
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1, 0x03);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xc7);
        outb(COM1 + 4, 0x0b);
    }
}

pub fn early_write(bytes: &[u8]) {
    initialize();
    for &byte in bytes {
        let _ = write_byte(byte);
    }
}

pub(super) fn present(bytes: &[u8]) -> Result<(), BaseError> {
    if bytes.len() > MAX_PRESENT_BYTES {
        return Err(BaseError::PayloadTooLarge);
    }
    for &byte in b"CONDUIT_SERIAL_PRESENT " {
        write_byte(byte)?;
    }
    for &byte in bytes {
        write_byte(byte)?;
    }
    write_byte(b'\n')
}

fn write_byte(byte: u8) -> Result<(), BaseError> {
    let mut spins = 0;
    while unsafe { inb(COM1 + 5) } & 0x20 == 0 {
        spins += 1;
        if spins == SERIAL_SPIN_LIMIT {
            return Err(BaseError::Unavailable);
        }
        core::hint::spin_loop();
    }
    unsafe { outb(COM1, byte) };
    Ok(())
}

pub(super) fn write_decimal(mut value: u64) {
    let mut digits = [0u8; 20];
    let mut index = digits.len();
    loop {
        index -= 1;
        digits[index] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    early_write(&digits[index..]);
}
