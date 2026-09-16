//! Bounded IA-32 Multiboot 1 handoff facts.

#![cfg_attr(test, allow(dead_code))]

use super::Firmware;

const MULTIBOOT1_BOOTLOADER_MAGIC: u32 = 0x2bad_b002;
const MULTIBOOT1_CMDLINE_PRESENT: u32 = 1 << 2;
const MULTIBOOT1_MODULES_PRESENT: u32 = 1 << 3;
const MULTIBOOT1_CMDLINE_OFFSET: usize = 16;
const MULTIBOOT1_MODULE_COUNT_OFFSET: usize = 20;
const MULTIBOOT1_MODULE_ADDRESS_OFFSET: usize = 24;
const MAX_FIRMWARE_CMDLINE_BYTES: usize = 32;
const MAX_MODULES: usize = 16;
const MAX_MODULE_COMMAND_BYTES: usize = 64;
const MODULE_ENTRY_BYTES: usize = 16;
const SPORE_MODULE_BYTES: usize = 4096;
const SPORE_MODULE_COMMAND: &[u8] = b"conduit.spore/native-media-provision@1";

/// Reads the bounded firmware fact passed by the pinned Limine Multiboot 1
/// entry. The config derives this value from Limine's current `${FW_TYPE}`
/// built-in instead of assigning a firmware identity in the kernel.
///
/// # Safety
///
/// `info_address` must be the Multiboot information pointer supplied in EBX
/// alongside `magic`. The bootloader owns that memory for the duration of this
/// call and any advertised command-line pointer must remain readable through
/// its terminating NUL byte within `MAX_FIRMWARE_CMDLINE_BYTES`.
pub unsafe fn firmware_from_multiboot1(
    magic: u32,
    info_address: u32,
) -> Result<Firmware, Multiboot1Error> {
    if magic != MULTIBOOT1_BOOTLOADER_MAGIC {
        return Err(Multiboot1Error::WrongMagic);
    }
    if info_address == 0 {
        return Err(Multiboot1Error::MissingInformation);
    }
    let info = info_address as usize as *const u8;
    let flags = unsafe { (info.cast::<u32>()).read_unaligned() };
    if flags & MULTIBOOT1_CMDLINE_PRESENT == 0 {
        return Err(Multiboot1Error::MissingCommandLine);
    }
    let command_line_address = unsafe {
        info.add(MULTIBOOT1_CMDLINE_OFFSET)
            .cast::<u32>()
            .read_unaligned()
    };
    if command_line_address == 0 {
        return Err(Multiboot1Error::MissingCommandLine);
    }
    let command_line = command_line_address as usize as *const u8;
    let mut bytes = [0_u8; MAX_FIRMWARE_CMDLINE_BYTES];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = unsafe { command_line.add(index).read() };
        if *byte == 0 {
            return decode_firmware_cmdline(&bytes[..index]);
        }
    }
    Err(Multiboot1Error::CommandLineTooLong)
}

/// Finds the one fixed ConduitOS spore module in a Multiboot 1 handoff.
///
/// # Safety
///
/// `info_address` and every module pointer selected from it must be the
/// bootloader-owned, identity-mapped values supplied with `magic`, and remain
/// readable for the duration of the boot.
pub unsafe fn spore_module_from_multiboot1(
    magic: u32,
    info_address: u32,
) -> Result<Option<&'static [u8]>, Multiboot1Error> {
    if magic != MULTIBOOT1_BOOTLOADER_MAGIC {
        return Err(Multiboot1Error::WrongMagic);
    }
    if info_address == 0 {
        return Err(Multiboot1Error::MissingInformation);
    }
    let info = info_address as usize as *const u8;
    let flags = unsafe { info.cast::<u32>().read_unaligned() };
    if flags & MULTIBOOT1_MODULES_PRESENT == 0 {
        return Ok(None);
    }
    let count = unsafe {
        info.add(MULTIBOOT1_MODULE_COUNT_OFFSET)
            .cast::<u32>()
            .read_unaligned()
    } as usize;
    if count > MAX_MODULES {
        return Err(Multiboot1Error::TooManyModules);
    }
    let entries = unsafe {
        info.add(MULTIBOOT1_MODULE_ADDRESS_OFFSET)
            .cast::<u32>()
            .read_unaligned()
    };
    if count != 0 && entries == 0 {
        return Err(Multiboot1Error::InvalidModule);
    }
    let mut selected = None;
    for index in 0..count {
        let entry = (entries as usize as *const u8).wrapping_add(index * MODULE_ENTRY_BYTES);
        let start = unsafe { entry.cast::<u32>().read_unaligned() };
        let end = unsafe { entry.add(4).cast::<u32>().read_unaligned() };
        let command = unsafe { entry.add(8).cast::<u32>().read_unaligned() };
        if command == 0 || !unsafe { command_equals(command, SPORE_MODULE_COMMAND) }? {
            continue;
        }
        if selected.is_some() || end.checked_sub(start) != Some(SPORE_MODULE_BYTES as u32) {
            return Err(Multiboot1Error::InvalidModule);
        }
        let bytes =
            unsafe { core::slice::from_raw_parts(start as usize as *const u8, SPORE_MODULE_BYTES) };
        selected = Some(bytes);
    }
    Ok(selected)
}

unsafe fn command_equals(address: u32, expected: &[u8]) -> Result<bool, Multiboot1Error> {
    let command = address as usize as *const u8;
    for index in 0..MAX_MODULE_COMMAND_BYTES {
        let byte = unsafe { command.add(index).read() };
        if byte == 0 {
            return Ok(index == expected.len());
        }
        if expected.get(index) != Some(&byte) {
            return Ok(false);
        }
    }
    Err(Multiboot1Error::ModuleCommandTooLong)
}

fn decode_firmware_cmdline(bytes: &[u8]) -> Result<Firmware, Multiboot1Error> {
    match bytes {
        b"firmware=BIOS" => Ok(Firmware::X86Bios),
        b"firmware=UEFI" => Ok(Firmware::Uefi32),
        _ => Err(Multiboot1Error::UnsupportedFirmware),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Multiboot1Error {
    WrongMagic,
    MissingInformation,
    MissingCommandLine,
    CommandLineTooLong,
    UnsupportedFirmware,
    TooManyModules,
    InvalidModule,
    ModuleCommandTooLong,
}

impl Multiboot1Error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WrongMagic => "multiboot1-magic-invalid",
            Self::MissingInformation => "multiboot1-information-missing",
            Self::MissingCommandLine => "multiboot1-command-line-missing",
            Self::CommandLineTooLong => "multiboot1-command-line-too-long",
            Self::UnsupportedFirmware => "multiboot1-firmware-unsupported",
            Self::TooManyModules => "multiboot1-module-count-invalid",
            Self::InvalidModule => "multiboot1-spore-module-invalid",
            Self::ModuleCommandTooLong => "multiboot1-module-command-too-long",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_limine_firmware_values_remain_distinct() {
        assert_eq!(
            decode_firmware_cmdline(b"firmware=BIOS"),
            Ok(Firmware::X86Bios)
        );
        assert_eq!(
            decode_firmware_cmdline(b"firmware=UEFI"),
            Ok(Firmware::Uefi32)
        );
    }

    #[test]
    fn absent_ambiguous_or_extended_values_refuse() {
        for value in [
            &b""[..],
            &b"BIOS"[..],
            &b"firmware=EFI"[..],
            &b"firmware=UEFI extra"[..],
        ] {
            assert_eq!(
                decode_firmware_cmdline(value),
                Err(Multiboot1Error::UnsupportedFirmware)
            );
        }
    }
}
