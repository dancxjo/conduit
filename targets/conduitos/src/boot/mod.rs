//! Bootloader-neutral bounded boot truth.

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
))]
mod limine;
#[cfg(any(test, target_arch = "x86"))]
mod multiboot1;
mod observation;

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
))]
pub use limine::{executable_physical_address, framebuffer_display, normalize_boot};
#[cfg(target_arch = "x86")]
pub use multiboot1::firmware_from_multiboot1;
pub use observation::{
    BootArtifact, BootError, BootNormalizer, BootRecord, Firmware, MAX_ARTIFACTS,
    MAX_COMMAND_LINE_BYTES, MAX_FRAMEBUFFERS, MAX_MEMORY_REGIONS, MemoryKind, MemoryRegion,
    RuntimeArena,
};
