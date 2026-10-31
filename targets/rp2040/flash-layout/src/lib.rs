//! Checked physical geometry, not flash possession or execution authority.
//! The platform must retain the sole FLASH owner and schedule bounded writes
//! before constructing a writable region adapter. These ranges grant no access.
#![no_std]

#[cfg(test)]
extern crate std;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Region {
    offset: u32,
    bytes: u32,
}
impl Region {
    pub const fn offset(self) -> u32 {
        self.offset
    }
    pub const fn address(self) -> u32 {
        FLASH_START + self.offset
    }
    pub const fn bytes(self) -> u32 {
        self.bytes
    }
    pub const fn contains(self, offset: u32, bytes: u32) -> bool {
        if bytes == 0 || offset < self.offset {
            return false;
        }
        match offset.checked_add(bytes) {
            Some(end) => end <= self.offset + self.bytes,
            None => false,
        }
    }
}
include!(concat!(env!("OUT_DIR"), "/layout.rs"));

pub const LINKER_MEMORY_X: &str = include_str!(concat!(env!("OUT_DIR"), "/memory.x"));
pub const LAYOUT_MODULE: &str = include_str!("../../deployment/browser/flash-layout.mjs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageRegion {
    Firmware,
    BodyBootstrap,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Alignment,
    ReservedOrOutsideFlash,
}
/// A deployment payload may replace firmware or the Body bootstrap, never
/// retained credentials. Classification does not authorize either replacement.
pub fn classify_image_page(address: u32, bytes: u32) -> Result<ImageRegion, Refusal> {
    if bytes != PAGE_BYTES || !address.is_multiple_of(PAGE_BYTES) {
        return Err(Refusal::Alignment);
    }
    let offset = address
        .checked_sub(FLASH_START)
        .ok_or(Refusal::ReservedOrOutsideFlash)?;
    if FIRMWARE.contains(offset, bytes) {
        Ok(ImageRegion::Firmware)
    } else if BOOTSTRAP.contains(offset, bytes) {
        Ok(ImageRegion::BodyBootstrap)
    } else {
        Err(Refusal::ReservedOrOutsideFlash)
    }
}

mod uf2;
pub use uf2::{Uf2Refusal, Uf2Validator, UF2_BLOCK_BYTES};

#[cfg(test)]
mod tests;
