//! Bounded structured boot Sign formatting.

use core::fmt::{self, Write};

use crate::{boot::BootRecord, identity::BootIdentities};

pub const BOOT_SIGN_SCHEMA: &str = "conduit.conduitos.boot-sign/v1";
pub const MAX_BOOT_SIGN_BYTES: usize = 1024;

pub struct FixedText {
    bytes: [u8; MAX_BOOT_SIGN_BYTES],
    len: usize,
}

impl FixedText {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_BOOT_SIGN_BYTES],
            len: 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl Default for FixedText {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for FixedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.len.checked_add(value.len()).ok_or(fmt::Error)?;
        let target = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        target.copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

pub fn accepted(
    record: &BootRecord,
    identities: &BootIdentities,
    build_id: &str,
    image_id: &str,
) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    write!(
        output,
        "CONDUIT_BOOT_SIGN {{\"schema\":\"{BOOT_SIGN_SCHEMA}\",\"status\":\"accepted\",\"arch\":\"{}\",\"firmware\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"limine\":\"12.5.2\",\"qemu_profile\":\"q35-single-cpu-64m-headless\",\"host_id\":\"",
        crate::arch::ARCHITECTURE,
        record.firmware.as_str(),
        build_id,
        image_id,
    )?;
    write_hex(&mut output, &identities.host)?;
    output.write_str("\",\"boot_id\":\"")?;
    write_hex(&mut output, &identities.boot)?;
    writeln!(
        output,
        "\",\"memory_regions\":{},\"artifacts\":{},\"framebuffers\":{},\"command_line_bytes\":{},\"runtime_arena_bytes\":{}}}",
        record.memory_region_count,
        record.artifact_count,
        record.framebuffer_count,
        record.command_line_bytes,
        record.runtime_arena.length,
    )?;
    Ok(output)
}

pub fn refused(reason: &str) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    writeln!(
        output,
        "CONDUIT_BOOT_SIGN {{\"schema\":\"{BOOT_SIGN_SCHEMA}\",\"status\":\"refused\",\"reason\":\"{reason}\"}}"
    )?;
    Ok(output)
}

fn write_hex(output: &mut FixedText, bytes: &[u8]) -> fmt::Result {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.write_char(HEX[(byte >> 4) as usize] as char)?;
        output.write_char(HEX[(byte & 0x0f) as usize] as char)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::{Firmware, RuntimeArena};

    #[test]
    fn accepted_sign_is_bounded_and_machine_readable() {
        let record = BootRecord {
            firmware: Firmware::Uefi64,
            timestamp: 1,
            hhdm_offset: 2,
            image_physical_start: 3,
            image_length: 4,
            memory_region_count: 5,
            artifact_count: 0,
            framebuffer_count: 0,
            command_line_bytes: 0,
            runtime_arena: RuntimeArena {
                physical_start: 6,
                length: 262_144,
            },
        };
        let output = accepted(
            &record,
            &BootIdentities {
                host: [0xaa; 32],
                boot: [0xbb; 32],
            },
            "build",
            "image",
        )
        .unwrap();
        let text = core::str::from_utf8(output.as_bytes()).unwrap();
        assert!(text.contains("\"status\":\"accepted\""));
        assert!(text.contains(&"aa".repeat(32)));
        assert!(text.len() <= MAX_BOOT_SIGN_BYTES);
    }
}
