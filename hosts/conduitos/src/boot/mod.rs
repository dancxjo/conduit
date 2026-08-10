//! Bootloader-neutral bounded boot truth.

#[cfg(not(target_arch = "x86"))]
mod limine;
mod observation;

#[cfg(not(target_arch = "x86"))]
pub use limine::{executable_physical_address, normalize_boot};
pub use observation::{
    BootArtifact, BootError, BootNormalizer, BootRecord, Firmware, MAX_ARTIFACTS,
    MAX_COMMAND_LINE_BYTES, MAX_FRAMEBUFFERS, MAX_MEMORY_REGIONS, MemoryKind, MemoryRegion,
    RuntimeArena,
};
