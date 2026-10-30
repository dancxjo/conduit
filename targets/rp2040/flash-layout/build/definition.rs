use serde::Deserialize;
use std::{format, string::String};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    schema: String,
    flash_start: u32,
    flash_bytes: u32,
    erase_bytes: u32,
    page_bytes: u32,
    boot2_bytes: u32,
    credentials_bytes: u32,
    bootstrap_bytes: u32,
}
impl Layout {
    pub fn decode(source: &str) -> Result<Self, &'static str> {
        if source.len() > 1024 {
            return Err("layout extent");
        }
        // The browser consumes this same frozen data module. Accept only its
        // exact literal wrapper, never evaluate JavaScript during preparation.
        let json = source
            .trim()
            .strip_prefix("export default Object.freeze(")
            .and_then(|source| source.strip_suffix(");"))
            .ok_or("layout literal wrapper")?;
        let value: Self = serde_json::from_str(json).map_err(|_| "layout encoding")?;
        // This layout belongs to the existing 2 MiB Pico/Pico W target, not to
        // arbitrary RP2040 flash chips. A different board needs a reviewed layout.
        if value.schema != "conduit.rp2040/flash-layout@1"
            || value.flash_start != 0x1000_0000
            || value.flash_bytes != 2 * 1024 * 1024
            || value.erase_bytes != 4096
            || value.page_bytes != 256
            || value.boot2_bytes != 256
            || value.credentials_bytes != 2 * value.erase_bytes
            || value.bootstrap_bytes != value.erase_bytes
        {
            return Err("unsupported Pico geometry or region contract");
        }
        Ok(value)
    }
    fn bootstrap_offset(&self) -> u32 {
        self.flash_bytes - self.bootstrap_bytes
    }
    fn credentials_offset(&self) -> u32 {
        self.bootstrap_offset() - self.credentials_bytes
    }
    pub fn rust(&self) -> String {
        format!(
            "pub const FLASH_START:u32={};\npub const FLASH_BYTES:u32={};\npub const ERASE_BYTES:u32={};\npub const PAGE_BYTES:u32={};\npub const FIRMWARE:Region=Region{{offset:0,bytes:{}}};\npub const CREDENTIALS:Region=Region{{offset:{},bytes:{}}};\npub const BOOTSTRAP:Region=Region{{offset:{},bytes:{}}};\n",
            self.flash_start, self.flash_bytes, self.erase_bytes, self.page_bytes,
            self.credentials_offset(), self.credentials_offset(), self.credentials_bytes,
            self.bootstrap_offset(), self.bootstrap_bytes,
        )
    }
    pub fn linker(&self) -> String {
        format!(
            "/* Generated from the checked RP2040 flash layout. */\nMEMORY\n{{\n  BOOT2 : ORIGIN = {:#x}, LENGTH = {:#x}\n  FLASH : ORIGIN = {:#x}, LENGTH = {:#x}\n  CREDENTIALS : ORIGIN = {:#x}, LENGTH = {:#x}\n  BODY_BOOTSTRAP : ORIGIN = {:#x}, LENGTH = {:#x}\n  RAM : ORIGIN = 0x20000000, LENGTH = 264K\n}}\n\nSECTIONS {{\n  .boot2 ORIGIN(BOOT2) : {{ KEEP(*(.boot2)); }} > BOOT2\n}} INSERT BEFORE .text;\n\n_stack_start = ORIGIN(RAM) + LENGTH(RAM);\n",
            self.flash_start, self.boot2_bytes,
            self.flash_start + self.boot2_bytes, self.credentials_offset() - self.boot2_bytes,
            self.flash_start + self.credentials_offset(), self.credentials_bytes,
            self.flash_start + self.bootstrap_offset(), self.bootstrap_bytes,
        )
    }
}
