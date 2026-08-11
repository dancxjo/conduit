pub const MAX_MEMORY_REGIONS: usize = 128;
pub const MAX_ARTIFACTS: usize = 16;
pub const MAX_FRAMEBUFFERS: usize = 8;
pub const MAX_COMMAND_LINE_BYTES: usize = 256;
#[cfg(not(feature = "hotplug-proof"))]
pub const MIN_RUNTIME_ARENA_BYTES: u64 = 1024 * 1024;
#[cfg(feature = "hotplug-proof")]
pub const MIN_RUNTIME_ARENA_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firmware {
    X86Bios,
    Uefi32,
    Uefi64,
    Sbi,
}

impl Firmware {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X86Bios => "x86-bios",
            Self::Uefi32 => "uefi32",
            Self::Uefi64 => "uefi64",
            Self::Sbi => "sbi",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    Bad,
    BootloaderReclaimable,
    ExecutableAndArtifacts,
    Framebuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegion {
    pub base: u64,
    pub length: u64,
    pub kind: MemoryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootArtifact {
    pub physical_start: u64,
    pub length: u64,
    pub path_hash: u64,
    pub command_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeArena {
    pub physical_start: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootRecord {
    pub firmware: Firmware,
    pub timestamp: u64,
    pub hhdm_offset: u64,
    pub image_physical_start: u64,
    pub image_length: u64,
    pub memory_region_count: u16,
    pub artifact_count: u16,
    pub framebuffer_count: u8,
    pub command_line_bytes: u16,
    pub runtime_arena: RuntimeArena,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootError {
    UnsupportedLimineRevision,
    MissingMemoryMap,
    MissingFirmware,
    MissingHhdm,
    MissingExecutableAddress,
    MissingExecutableFile,
    MissingBootTimestamp,
    TooManyMemoryRegions,
    TooManyArtifacts,
    TooManyFramebuffers,
    MalformedMemoryRange,
    MalformedArtifactRange,
    MalformedImageRange,
    MalformedHhdmConversion,
    OverlappingMemoryRegions,
    OverlappingArtifacts,
    CommandLineTooLong,
    MalformedCommandLine,
    RuntimeArenaUnavailable,
}

impl BootError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedLimineRevision => "unsupported-limine-revision",
            Self::MissingMemoryMap => "missing-memory-map",
            Self::MissingFirmware => "missing-firmware",
            Self::MissingHhdm => "missing-hhdm",
            Self::MissingExecutableAddress => "missing-executable-address",
            Self::MissingExecutableFile => "missing-executable-file",
            Self::MissingBootTimestamp => "missing-boot-timestamp",
            Self::TooManyMemoryRegions => "too-many-memory-regions",
            Self::TooManyArtifacts => "too-many-artifacts",
            Self::TooManyFramebuffers => "too-many-framebuffers",
            Self::MalformedMemoryRange => "malformed-memory-range",
            Self::MalformedArtifactRange => "malformed-artifact-range",
            Self::MalformedImageRange => "malformed-image-range",
            Self::MalformedHhdmConversion => "malformed-hhdm-conversion",
            Self::OverlappingMemoryRegions => "overlapping-memory-regions",
            Self::OverlappingArtifacts => "overlapping-artifacts",
            Self::CommandLineTooLong => "command-line-too-long",
            Self::MalformedCommandLine => "malformed-command-line",
            Self::RuntimeArenaUnavailable => "runtime-arena-unavailable",
        }
    }
}

pub struct BootNormalizer {
    firmware: Firmware,
    timestamp: u64,
    hhdm_offset: u64,
    image_start: u64,
    image_length: u64,
    previous_region_end: Option<u64>,
    previous_artifact_end: Option<u64>,
    region_count: u16,
    artifact_count: u16,
    framebuffer_count: u8,
    command_line_bytes: u16,
    runtime_arena: Option<RuntimeArena>,
}

impl BootNormalizer {
    pub fn new(
        firmware: Firmware,
        timestamp: u64,
        hhdm_offset: u64,
        image_start: u64,
        image_length: u64,
    ) -> Result<Self, BootError> {
        checked_end(image_start, image_length).ok_or(BootError::MalformedImageRange)?;
        if image_length == 0 {
            return Err(BootError::MalformedImageRange);
        }
        Ok(Self {
            firmware,
            timestamp,
            hhdm_offset,
            image_start,
            image_length,
            previous_region_end: None,
            previous_artifact_end: None,
            region_count: 0,
            artifact_count: 0,
            framebuffer_count: 0,
            command_line_bytes: 0,
            runtime_arena: None,
        })
    }

    pub fn push_region(&mut self, region: MemoryRegion) -> Result<(), BootError> {
        if self.region_count as usize == MAX_MEMORY_REGIONS {
            return Err(BootError::TooManyMemoryRegions);
        }
        let end = checked_end(region.base, region.length).ok_or(BootError::MalformedMemoryRange)?;
        if region.length == 0 {
            return Err(BootError::MalformedMemoryRange);
        }
        if self
            .previous_region_end
            .is_some_and(|previous| region.base < previous)
        {
            return Err(BootError::OverlappingMemoryRegions);
        }
        self.previous_region_end = Some(end);
        self.region_count += 1;

        if self.runtime_arena.is_none()
            && region.kind == MemoryKind::Usable
            && region.length >= MIN_RUNTIME_ARENA_BYTES
        {
            self.runtime_arena = Some(RuntimeArena {
                physical_start: region.base,
                length: MIN_RUNTIME_ARENA_BYTES,
            });
        }
        Ok(())
    }

    pub fn push_artifact(&mut self, artifact: BootArtifact) -> Result<(), BootError> {
        if self.artifact_count as usize == MAX_ARTIFACTS {
            return Err(BootError::TooManyArtifacts);
        }
        let end = checked_end(artifact.physical_start, artifact.length)
            .ok_or(BootError::MalformedArtifactRange)?;
        if artifact.length == 0 {
            return Err(BootError::MalformedArtifactRange);
        }
        if self
            .previous_artifact_end
            .is_some_and(|previous| artifact.physical_start < previous)
        {
            return Err(BootError::OverlappingArtifacts);
        }
        self.previous_artifact_end = Some(end);
        self.artifact_count += 1;
        Ok(())
    }

    pub fn set_framebuffer_count(&mut self, count: usize) -> Result<(), BootError> {
        self.framebuffer_count = count
            .try_into()
            .map_err(|_| BootError::TooManyFramebuffers)?;
        if count > MAX_FRAMEBUFFERS {
            return Err(BootError::TooManyFramebuffers);
        }
        Ok(())
    }

    pub fn set_command_line(&mut self, bytes: &[u8]) -> Result<(), BootError> {
        if bytes.len() > MAX_COMMAND_LINE_BYTES {
            return Err(BootError::CommandLineTooLong);
        }
        if core::str::from_utf8(bytes).is_err() || bytes.contains(&0) {
            return Err(BootError::MalformedCommandLine);
        }
        self.command_line_bytes = bytes.len() as u16;
        Ok(())
    }

    pub fn finish(self) -> Result<BootRecord, BootError> {
        Ok(BootRecord {
            firmware: self.firmware,
            timestamp: self.timestamp,
            hhdm_offset: self.hhdm_offset,
            image_physical_start: self.image_start,
            image_length: self.image_length,
            memory_region_count: self.region_count,
            artifact_count: self.artifact_count,
            framebuffer_count: self.framebuffer_count,
            command_line_bytes: self.command_line_bytes,
            runtime_arena: self
                .runtime_arena
                .ok_or(BootError::RuntimeArenaUnavailable)?,
        })
    }
}

#[cfg(not(target_arch = "x86"))]
pub fn hhdm_to_physical(address: u64, offset: u64) -> Result<u64, BootError> {
    address
        .checked_sub(offset)
        .ok_or(BootError::MalformedHhdmConversion)
}

#[cfg(not(target_arch = "x86"))]
pub fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

const fn checked_end(start: u64, length: u64) -> Option<u64> {
    start.checked_add(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalizer() -> BootNormalizer {
        BootNormalizer::new(
            Firmware::Uefi64,
            1,
            0xffff_8000_0000_0000,
            0x20_0000,
            0x10_000,
        )
        .unwrap()
    }

    #[test]
    fn accepts_finite_sorted_boot_truth() {
        let mut value = normalizer();
        value
            .push_region(MemoryRegion {
                base: 0x1000,
                length: MIN_RUNTIME_ARENA_BYTES,
                kind: MemoryKind::Usable,
            })
            .unwrap();
        value.set_framebuffer_count(0).unwrap();
        value.set_command_line(b"").unwrap();
        let record = value.finish().unwrap();
        assert_eq!(record.memory_region_count, 1);
        assert_eq!(record.runtime_arena.physical_start, 0x1000);
    }

    #[test]
    fn rejects_overlap_overflow_and_missing_arena_distinctly() {
        let mut overlap = normalizer();
        overlap
            .push_region(MemoryRegion {
                base: 0x1000,
                length: 0x2000,
                kind: MemoryKind::Reserved,
            })
            .unwrap();
        assert_eq!(
            overlap.push_region(MemoryRegion {
                base: 0x2000,
                length: 0x1000,
                kind: MemoryKind::Reserved,
            }),
            Err(BootError::OverlappingMemoryRegions)
        );

        assert_eq!(
            normalizer().push_region(MemoryRegion {
                base: u64::MAX,
                length: 2,
                kind: MemoryKind::Usable,
            }),
            Err(BootError::MalformedMemoryRange)
        );
        assert_eq!(
            normalizer().finish(),
            Err(BootError::RuntimeArenaUnavailable)
        );
    }

    #[test]
    fn rejects_oversized_or_malformed_command_lines() {
        let mut value = normalizer();
        assert_eq!(
            value.set_command_line(&[b'x'; MAX_COMMAND_LINE_BYTES + 1]),
            Err(BootError::CommandLineTooLong)
        );
        assert_eq!(
            value.set_command_line(&[0xff]),
            Err(BootError::MalformedCommandLine)
        );
        assert_eq!(
            value.set_command_line(b"a\0b"),
            Err(BootError::MalformedCommandLine)
        );
    }

    #[test]
    fn hhdm_conversion_fails_closed() {
        assert_eq!(
            hhdm_to_physical(9, 10),
            Err(BootError::MalformedHhdmConversion)
        );
        assert_eq!(hhdm_to_physical(11, 10), Ok(1));
    }

    #[test]
    fn finite_region_and_artifact_caps_refuse_without_truncation() {
        let mut regions = normalizer();
        for index in 0..MAX_MEMORY_REGIONS {
            regions
                .push_region(MemoryRegion {
                    base: 0x10_0000 + index as u64 * 0x1000,
                    length: 0x1000,
                    kind: MemoryKind::Reserved,
                })
                .unwrap();
        }
        assert_eq!(
            regions.push_region(MemoryRegion {
                base: 0x10_0000 + MAX_MEMORY_REGIONS as u64 * 0x1000,
                length: 0x1000,
                kind: MemoryKind::Reserved,
            }),
            Err(BootError::TooManyMemoryRegions)
        );

        let mut artifacts = normalizer();
        for index in 0..MAX_ARTIFACTS {
            artifacts
                .push_artifact(BootArtifact {
                    physical_start: 0x30_0000 + index as u64 * 0x1000,
                    length: 0x1000,
                    path_hash: index as u64,
                    command_hash: 0,
                })
                .unwrap();
        }
        assert_eq!(
            artifacts.push_artifact(BootArtifact {
                physical_start: 0x30_0000 + MAX_ARTIFACTS as u64 * 0x1000,
                length: 0x1000,
                path_hash: 0,
                command_hash: 0,
            }),
            Err(BootError::TooManyArtifacts)
        );
    }

    #[test]
    fn artifact_overlap_and_framebuffer_overflow_are_distinct() {
        let mut value = normalizer();
        value
            .push_artifact(BootArtifact {
                physical_start: 0x30_0000,
                length: 0x2000,
                path_hash: 1,
                command_hash: 2,
            })
            .unwrap();
        assert_eq!(
            value.push_artifact(BootArtifact {
                physical_start: 0x30_1000,
                length: 0x1000,
                path_hash: 3,
                command_hash: 4,
            }),
            Err(BootError::OverlappingArtifacts)
        );
        assert_eq!(
            value.set_framebuffer_count(MAX_FRAMEBUFFERS + 1),
            Err(BootError::TooManyFramebuffers)
        );
    }
}
