use super::*;

#[test]
fn runtime_arena_matches_the_compiled_presentation_shape() {
    #[cfg(feature = "native-compositor")]
    assert_eq!(MIN_RUNTIME_ARENA_BYTES, 16 * 1024 * 1024);
    #[cfg(not(feature = "native-compositor"))]
    assert_eq!(MIN_RUNTIME_ARENA_BYTES, 8 * 1024 * 1024);
}

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
#[test]
fn unordered_modules_are_admitted_but_overlap_with_any_prior_module_refuses() {
    let mut value = normalizer();
    let artifact = |base, length| BootArtifact {
        physical_start: base,
        length,
        path_hash: base,
        command_hash: 0,
    };
    for base in [0x50_0000, 0x30_0000, 0x40_0000] {
        value.push_artifact(artifact(base, 0x1000)).unwrap();
    }
    assert_eq!(
        value.push_artifact(artifact(0x50_0800, 0x1000)),
        Err(BootError::OverlappingArtifacts)
    );
    assert_eq!(
        value.push_artifact(artifact(0x2f_f800, 0x1000)),
        Err(BootError::OverlappingArtifacts)
    );
    value.push_artifact(artifact(0x50_1000, 0x1000)).unwrap();
    value
        .push_region(MemoryRegion {
            base: 0x60_0000,
            length: MIN_RUNTIME_ARENA_BYTES,
            kind: MemoryKind::Usable,
        })
        .unwrap();
    assert_eq!(value.finish().unwrap().artifact_count, 4);
}

#[test]
fn selected_preparation_budget_requires_one_exact_usable_range() {
    let budget = 64 * 1024 * 1024;
    let mut value = normalizer().require_arena_bytes(budget).unwrap();
    value
        .push_region(MemoryRegion {
            base: 0x400000,
            length: budget / 2,
            kind: MemoryKind::Usable,
        })
        .unwrap();
    value
        .push_region(MemoryRegion {
            base: 0x4000000,
            length: budget,
            kind: MemoryKind::Usable,
        })
        .unwrap();
    let arena = value.finish().unwrap().runtime_arena;
    assert_eq!(arena.physical_start, 0x4000000);
    assert_eq!(arena.length, budget);
    let mut missing = normalizer().require_arena_bytes(budget).unwrap();
    missing
        .push_region(MemoryRegion {
            base: 0x400000,
            length: budget / 2,
            kind: MemoryKind::Usable,
        })
        .unwrap();
    assert_eq!(missing.finish(), Err(BootError::RuntimeArenaUnavailable));
}

#[test]
fn unsupported_or_late_preparation_budgets_refuse_before_changing_boot_truth() {
    for bytes in [
        0,
        MIN_RUNTIME_ARENA_BYTES - 1,
        MIN_RUNTIME_ARENA_BYTES + 1,
        64 * 1024 * 1024 + 4096,
    ] {
        assert!(matches!(
            normalizer().require_arena_bytes(bytes),
            Err(BootError::UnsupportedRuntimeArenaBudget)
        ));
    }
    let mut value = normalizer();
    value
        .push_region(MemoryRegion {
            base: 0x400000,
            length: MIN_RUNTIME_ARENA_BYTES,
            kind: MemoryKind::Usable,
        })
        .unwrap();
    assert!(matches!(
        value.require_arena_bytes(64 * 1024 * 1024),
        Err(BootError::UnsupportedRuntimeArenaBudget)
    ));
}
