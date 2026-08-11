//! Sole adapter from Limine protocol types into bootloader-neutral boot truth.

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use limine::request::StackSizeRequest;
use limine::{
    BaseRevision,
    firmware_type::FirmwareType,
    memory_map::EntryType,
    request::{
        BootloaderInfoRequest, DateAtBootRequest, ExecutableAddressRequest,
        ExecutableCmdlineRequest, ExecutableFileRequest, FirmwareTypeRequest, FramebufferRequest,
        HhdmRequest, MemoryMapRequest, ModuleRequest,
    },
};

use super::observation::{
    BootArtifact, BootError, BootNormalizer, BootRecord, Firmware, MemoryKind, MemoryRegion,
    hhdm_to_physical, stable_hash,
};
use crate::display::{DisplayError, DisplayFormat, RawDisplay};

const PINNED_BOOTLOADER_NAME: &str = "Limine";
const PINNED_BOOTLOADER_VERSION: &str = "12.5.2";

#[used]
#[unsafe(link_section = ".requests")]
#[cfg(target_arch = "aarch64")]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(6);
#[used]
#[unsafe(link_section = ".requests")]
#[cfg(not(target_arch = "aarch64"))]
static BASE_REVISION: BaseRevision = BaseRevision::new();
#[used]
#[unsafe(link_section = ".requests")]
static BOOTLOADER_INFO: BootloaderInfoRequest = BootloaderInfoRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static FIRMWARE: FirmwareTypeRequest = FirmwareTypeRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP: MemoryMapRequest = MemoryMapRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static HHDM: HhdmRequest = HhdmRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_ADDRESS: ExecutableAddressRequest = ExecutableAddressRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_FILE: ExecutableFileRequest = ExecutableFileRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static MODULES: ModuleRequest = ModuleRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFERS: FramebufferRequest = FramebufferRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static DATE_AT_BOOT: DateAtBootRequest = DateAtBootRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_CMDLINE: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
#[cfg(target_arch = "aarch64")]
static AARCH64_STACK_SIZE: StackSizeRequest = StackSizeRequest::new().with_size(128 * 1024);
#[used]
#[unsafe(link_section = ".requests")]
#[cfg(target_arch = "x86_64")]
static X86_64_STACK_SIZE: StackSizeRequest = StackSizeRequest::new().with_size(1024 * 1024);

pub fn executable_physical_address(virtual_address: u64) -> Option<u64> {
    let response = EXECUTABLE_ADDRESS.get_response()?;
    virtual_address
        .checked_sub(response.virtual_base())?
        .checked_add(response.physical_base())
}

pub fn framebuffer_display() -> Result<RawDisplay, DisplayError> {
    let response = FRAMEBUFFERS.get_response().ok_or(DisplayError::Absent)?;
    let mut framebuffers = response.framebuffers();
    let framebuffer = framebuffers.next().ok_or(DisplayError::Absent)?;
    if framebuffers.next().is_some()
        || framebuffer.memory_model() != limine::framebuffer::MemoryModel::RGB
        || framebuffer.red_mask_size() != 8
        || framebuffer.green_mask_size() != 8
        || framebuffer.blue_mask_size() != 8
    {
        return Err(DisplayError::UnsupportedFormat);
    }
    let format = DisplayFormat {
        width: u32::try_from(framebuffer.width()).map_err(|_| DisplayError::InvalidExtent)?,
        height: u32::try_from(framebuffer.height()).map_err(|_| DisplayError::InvalidExtent)?,
        pitch: u32::try_from(framebuffer.pitch()).map_err(|_| DisplayError::InvalidExtent)?,
        bits_per_pixel: u8::try_from(framebuffer.bpp())
            .map_err(|_| DisplayError::UnsupportedFormat)?,
        red_shift: framebuffer.red_mask_shift(),
        green_shift: framebuffer.green_mask_shift(),
        blue_shift: framebuffer.blue_mask_shift(),
    };
    let address = core::ptr::NonNull::new(framebuffer.addr()).ok_or(DisplayError::Absent)?;
    let byte_len = format.byte_len()?;
    // SAFETY: Limine owns the selected framebuffer response and promises its
    // address and current-mode extent as writable framebuffer memory.
    unsafe { RawDisplay::new(address, byte_len, format) }
}

unsafe extern "C" {
    static __conduitos_image_start: u8;
    static __conduitos_image_end: u8;
}

pub fn normalize_boot() -> Result<BootRecord, BootError> {
    if !BASE_REVISION.is_supported() {
        return Err(BootError::UnsupportedLimineRevision);
    }
    let bootloader = BOOTLOADER_INFO
        .get_response()
        .ok_or(BootError::UnsupportedLimineRevision)?;
    if bootloader.name() != PINNED_BOOTLOADER_NAME
        || bootloader.version() != PINNED_BOOTLOADER_VERSION
    {
        return Err(BootError::UnsupportedLimineRevision);
    }
    let firmware = FIRMWARE
        .get_response()
        .ok_or(BootError::MissingFirmware)?
        .firmware_type();
    let firmware = if firmware == FirmwareType::X86_BIOS {
        Firmware::X86Bios
    } else if firmware == FirmwareType::UEFI_32 {
        Firmware::Uefi32
    } else if firmware == FirmwareType::UEFI_64 {
        Firmware::Uefi64
    } else if firmware == FirmwareType::SBI {
        Firmware::Sbi
    } else {
        return Err(BootError::MissingFirmware);
    };
    let hhdm = HHDM.get_response().ok_or(BootError::MissingHhdm)?.offset();
    let executable_address = EXECUTABLE_ADDRESS
        .get_response()
        .ok_or(BootError::MissingExecutableAddress)?;
    let _executable_file = EXECUTABLE_FILE
        .get_response()
        .ok_or(BootError::MissingExecutableFile)?;
    let timestamp = DATE_AT_BOOT
        .get_response()
        .ok_or(BootError::MissingBootTimestamp)?
        .timestamp()
        .as_secs();

    let virtual_start = core::ptr::addr_of!(__conduitos_image_start) as u64;
    let virtual_end = core::ptr::addr_of!(__conduitos_image_end) as u64;
    let image_length = virtual_end
        .checked_sub(virtual_start)
        .ok_or(BootError::MalformedImageRange)?;
    let image_offset = virtual_start
        .checked_sub(executable_address.virtual_base())
        .ok_or(BootError::MalformedImageRange)?;
    let image_start = executable_address
        .physical_base()
        .checked_add(image_offset)
        .ok_or(BootError::MalformedImageRange)?;

    let mut normalized = BootNormalizer::new(firmware, timestamp, hhdm, image_start, image_length)?;
    let memory = MEMORY_MAP
        .get_response()
        .ok_or(BootError::MissingMemoryMap)?;
    for entry in memory.entries() {
        normalized.push_region(MemoryRegion {
            base: entry.base,
            length: entry.length,
            kind: memory_kind(entry.entry_type),
        })?;
    }

    if let Some(response) = MODULES.get_response() {
        for file in response.modules() {
            normalized.push_artifact(BootArtifact {
                physical_start: hhdm_to_physical(file.addr() as u64, hhdm)?,
                length: file.size(),
                path_hash: stable_hash(file.path().to_bytes()),
                command_hash: stable_hash(file.string().to_bytes()),
            })?;
        }
    }
    let framebuffer_count = FRAMEBUFFERS
        .get_response()
        .map_or(0, |response| response.framebuffers().count());
    normalized.set_framebuffer_count(framebuffer_count)?;

    let command_line = EXECUTABLE_CMDLINE
        .get_response()
        .map_or(&[][..], |response| response.cmdline().to_bytes());
    normalized.set_command_line(command_line)?;
    normalized.finish()
}

fn memory_kind(kind: EntryType) -> MemoryKind {
    if kind == EntryType::USABLE {
        MemoryKind::Usable
    } else if kind == EntryType::ACPI_RECLAIMABLE {
        MemoryKind::AcpiReclaimable
    } else if kind == EntryType::ACPI_NVS {
        MemoryKind::AcpiNvs
    } else if kind == EntryType::BAD_MEMORY {
        MemoryKind::Bad
    } else if kind == EntryType::BOOTLOADER_RECLAIMABLE {
        MemoryKind::BootloaderReclaimable
    } else if kind == EntryType::EXECUTABLE_AND_MODULES {
        MemoryKind::ExecutableAndArtifacts
    } else if kind == EntryType::FRAMEBUFFER {
        MemoryKind::Framebuffer
    } else {
        MemoryKind::Reserved
    }
}
