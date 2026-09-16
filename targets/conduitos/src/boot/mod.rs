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
pub use limine::{executable_physical_address, framebuffer_display, normalize_boot, spore_module};
#[cfg(target_arch = "x86")]
pub use multiboot1::{firmware_from_multiboot1, spore_module_from_multiboot1};

/// Whether the normalized Boot carries no artifact other than the reviewed
/// Body-bound spore module.
pub fn has_only_optional_spore_artifact(record: &BootRecord) -> bool {
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    ))]
    let spore_is_normalized_artifact = spore_module().is_some();
    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    )))]
    let spore_is_normalized_artifact = false;

    has_only_optional_spore_artifact_count(record.artifact_count, spore_is_normalized_artifact)
}

const fn has_only_optional_spore_artifact_count(
    artifact_count: u16,
    spore_is_normalized_artifact: bool,
) -> bool {
    matches!(
        (artifact_count, spore_is_normalized_artifact),
        (0, false) | (1, true)
    )
}
pub use observation::{
    BootArtifact, BootError, BootNormalizer, BootRecord, Firmware, MAX_ARTIFACTS,
    MAX_COMMAND_LINE_BYTES, MAX_FRAMEBUFFERS, MAX_MEMORY_REGIONS, MemoryKind, MemoryRegion,
    RuntimeArena,
};

#[cfg(test)]
mod tests {
    use super::has_only_optional_spore_artifact_count;

    #[test]
    fn only_the_exact_optional_spore_artifact_is_representable() {
        assert!(has_only_optional_spore_artifact_count(0, false));
        assert!(has_only_optional_spore_artifact_count(1, true));
        assert!(!has_only_optional_spore_artifact_count(1, false));
        assert!(!has_only_optional_spore_artifact_count(0, true));
        assert!(!has_only_optional_spore_artifact_count(2, true));
    }
}
