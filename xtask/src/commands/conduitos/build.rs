use std::{fs, path::Path, process::Command};

use conduit_host_fabrication::{
    build_host_image, BuildInputs, BuildManifest, FabricationCatalog, HostBounds, HostProfile,
};

use crate::cli::GlobalOpts;

use super::{
    aarch64_a0, ia32_a2, loongarch64_a0,
    profile::{Paths, COMMON_BACKBONE_TARGETS},
    report::{git_head, sha256_file, BuildRecord},
    riscv64_a0, target_lowering, ConduitosArch, ConduitosError,
};

pub fn execute(arch: ConduitosArch, opts: &GlobalOpts) -> Result<BuildRecord, ConduitosError> {
    if arch == ConduitosArch::X86_64 {
        execute_embedded_profile(
            arch,
            include_str!("../../../../profiles/hosts/conduitos-native.profile.json"),
            opts,
        )
    } else {
        execute_with_features(arch, opts, &[], None)
    }
}

pub(super) fn execute_profile(
    manifest: &BuildManifest,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    let arch = match manifest.target.as_str() {
        "conduitos/x86_64/pc" => ConduitosArch::X86_64,
        "conduitos/aarch64/virt" => ConduitosArch::Aarch64,
        target => {
            return Err(ConduitosError::refusal(
                "unsupported-profile-target",
                target.to_owned(),
            ));
        }
    };
    let paths = Paths::new(arch)?;
    fs::create_dir_all(&paths.target)
        .map_err(|error| ConduitosError::refusal("build-output-unavailable", error.to_string()))?;
    let generated = paths.target.join("fabrication-record.rs");
    let lowering = target_lowering::lower(manifest)?;
    let source = format!(
        "pub const EMBEDDED_FABRICATION: FabricationRecord = FabricationRecord {{ schema: {schema:?}, profile_id: {profile:?}, build_id: {build:?}, image_binding: {binding:?}, target: {target:?}, implementations: {implementations}, facilities: {facilities}, resources: {resources}, bases: {bases}, drivers: {drivers}, presenters: {presenters}, proof_instrumentation: {proof_instrumentation}, presentation_surface_slots: {surface_slots}, presentation_surface_bytes: {surface_bytes}, runtime_arena_ceiling: {arena}, operation_slot_ceiling: {operations}, timer_slot_ceiling: {timers}, evidence_item_ceiling: {evidence} }};\n",
        schema = conduitos::fabrication::FABRICATION_SCHEMA,
        profile = manifest.profile_id,
        build = manifest.build_id,
        binding = manifest.image_id,
        target = manifest.target,
        implementations = lowering.implementations,
        facilities = lowering.facilities,
        resources = lowering.resources,
        bases = lowering.bases,
        drivers = lowering.drivers,
        presenters = lowering.presenters,
        proof_instrumentation = lowering.proof_instrumentation,
        surface_slots = lowering.presentation_surface_slots,
        surface_bytes = lowering.presentation_surface_bytes,
        arena = manifest.bounds.static_memory_bytes,
        operations = manifest.bounds.operation_slots,
        timers = manifest.bounds.timer_slots,
        evidence = manifest.bounds.evidence_items,
    );
    fs::write(&generated, source)
        .map_err(|error| ConduitosError::refusal("build-output-unavailable", error.to_string()))?;
    execute_with_features(
        arch,
        opts,
        &lowering.cargo_features,
        Some(ProfileFabrication {
            generated: &generated,
            build_id: &manifest.build_id,
            image_binding: &manifest.image_id,
        }),
    )
}

pub(super) fn execute_hotplug(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    execute_embedded_profile(
        arch,
        include_str!("../../../../profiles/hosts/conduitos-hotplug-proof.profile.json"),
        opts,
    )
}

pub(super) fn execute_proof(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    execute_embedded_profile(
        arch,
        include_str!("../../../../profiles/hosts/conduitos-proof.profile.json"),
        opts,
    )
}

fn execute_embedded_profile(
    arch: ConduitosArch,
    source: &str,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    if arch != ConduitosArch::X86_64 {
        return execute(arch, opts);
    }
    let manifest = resolve_embedded_profile(arch, source)?;
    execute_profile(&manifest, opts)
}

pub(super) fn proof_manifest(arch: ConduitosArch) -> Result<BuildManifest, ConduitosError> {
    resolve_embedded_profile(
        arch,
        include_str!("../../../../profiles/hosts/conduitos-proof.profile.json"),
    )
}

fn resolve_embedded_profile(
    arch: ConduitosArch,
    source: &str,
) -> Result<BuildManifest, ConduitosError> {
    if arch != ConduitosArch::X86_64 {
        return Err(ConduitosError::refusal(
            "unsupported-profile-target",
            "checked product PROFILE lowering currently owns x86_64 only",
        ));
    }
    let paths = Paths::new(arch)?;
    let profile: HostProfile = serde_json::from_str(source)
        .map_err(|error| ConduitosError::refusal("proof-profile-invalid", error.to_string()))?;
    let source_identity = git_head(&paths.root)?;
    let toolchain = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|error| ConduitosError::refusal("toolchain-unavailable", error.to_string()))?;
    if !toolchain.status.success() {
        return Err(ConduitosError::refusal(
            "toolchain-unavailable",
            toolchain.status.to_string(),
        ));
    }
    let toolchain_identity = String::from_utf8(toolchain.stdout)
        .map_err(|error| ConduitosError::refusal("toolchain-unavailable", error.to_string()))?;
    let (image, _) = build_host_image(
        profile,
        &FabricationCatalog::canonical(),
        &BuildInputs {
            source_identity,
            toolchain_identity: toolchain_identity.trim().into(),
            toolchain_available: true,
            maxima: HostBounds {
                static_memory_bytes: 512 * 1024 * 1024,
                heap_arena_bytes: 512 * 1024 * 1024,
                queue_items: 1_048_576,
                buffered_bytes: 512 * 1024 * 1024,
                active_instances: 65_536,
                operation_slots: 65_536,
                timer_slots: 65_536,
                line_sessions: 65_536,
                evidence_items: 1_048_576,
            },
        },
    )
    .map_err(|diagnostics| {
        ConduitosError::refusal("proof-profile-refused", format!("{diagnostics:?}"))
    })?;
    Ok(image.manifest)
}

fn execute_with_features(
    arch: ConduitosArch,
    opts: &GlobalOpts,
    features: &[&str],
    fabrication: Option<ProfileFabrication<'_>>,
) -> Result<BuildRecord, ConduitosError> {
    if arch == ConduitosArch::Ia32 {
        return ia32_a2::execute(opts);
    }
    if arch == ConduitosArch::Aarch64 && fabrication.is_none() {
        return aarch64_a0::execute(opts);
    }
    if arch == ConduitosArch::Riscv64 {
        return riscv64_a0::execute(opts);
    }
    if arch == ConduitosArch::Loongarch64 {
        return loongarch64_a0::execute(opts);
    }
    let paths = Paths::new(arch)?;
    let (binary, target) = match arch {
        ConduitosArch::X86_64 => ("conduitos", "x86_64-unknown-none"),
        ConduitosArch::Aarch64 => ("conduitos-aarch64-product", "aarch64-unknown-none"),
        _ => unreachable!("profile build architecture checked above"),
    };
    if opts.dry_run {
        println!(
            "cargo build -p conduitos --bin {binary} --target {target} --release --features {}",
            features.join(",")
        );
        return dry_record(arch, &paths);
    }
    fs::create_dir_all(&paths.target)
        .map_err(|error| ConduitosError::refusal("build-output-unavailable", error.to_string()))?;
    check_common_backbone(&paths, opts)?;
    let base_commit = git_head(&paths.root)?;
    let legacy_image_id = format!("conduitos-image/{base_commit}/{}/v1", arch.as_str());
    let build_id = fabrication.map_or(base_commit.as_str(), |item| item.build_id);
    let image_binding = fabrication.map_or(legacy_image_id.as_str(), |item| item.image_binding);
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "-p",
            "conduitos",
            "--bin",
            binary,
            "--target",
            target,
            "--release",
        ])
        .current_dir(&paths.root)
        .env("RUSTFLAGS", "-C relocation-model=static -C panic=abort")
        .env("CONDUITOS_BUILD_ID", build_id)
        .env("CONDUITOS_IMAGE_ID", image_binding);
    if let Some(fabrication) = fabrication {
        command.env("CONDUITOS_FABRICATION_RECORD", fabrication.generated);
    }
    if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }
    if opts.locked {
        command.arg("--locked");
    }
    let status = command.status().map_err(|error| {
        ConduitosError::refusal(
            "toolchain-unavailable",
            format!("cannot launch cargo: {error}"),
        )
    })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "compile-link-failed",
            status.to_string(),
        ));
    }
    let built = paths
        .root
        .join("target")
        .join(target)
        .join("release")
        .join(binary);
    fs::copy(&built, &paths.kernel)
        .map_err(|error| ConduitosError::refusal("build-output-unavailable", error.to_string()))?;
    assert_elf(&paths)?;
    let record = BuildRecord {
        schema: "conduit.conduitos.build/v1",
        base_commit,
        architecture: arch.as_str(),
        rust_target: target,
        limine_crate: "0.5.0",
        elf_sha256: sha256_file(&paths.kernel)?,
    };
    write_json(&paths.target.join("build.json"), &record)?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS ELF: {}", paths.kernel.display());
    }
    Ok(record)
}

#[derive(Clone, Copy)]
struct ProfileFabrication<'a> {
    generated: &'a Path,
    build_id: &'a str,
    image_binding: &'a str,
}

fn check_common_backbone(paths: &Paths, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    for target in COMMON_BACKBONE_TARGETS {
        let mut command = Command::new("cargo");
        command
            .args(["check", "-p", "conduitos", "--lib", "--target", target])
            .current_dir(&paths.root);
        if opts.locked {
            command.arg("--locked");
        }
        let status = command.status().map_err(|error| {
            ConduitosError::refusal(
                "matrix-toolchain-unavailable",
                format!("cannot check common backbone for {target}: {error}"),
            )
        })?;
        if !status.success() {
            return Err(ConduitosError::refusal(
                "matrix-common-backbone-failed",
                format!("shared ConduitOS backbone did not compile for {target}"),
            ));
        }
    }
    Ok(())
}

fn assert_elf(paths: &Paths) -> Result<(), ConduitosError> {
    let kernel = paths.kernel.to_str().ok_or_else(|| {
        ConduitosError::refusal("build-output-unavailable", "non-UTF-8 kernel path")
    })?;
    let program_headers = super::profile::command(
        "readelf",
        &["-lW", kernel],
        &paths.root,
        "readelf-unavailable",
    )?;
    let output = String::from_utf8_lossy(&program_headers.stdout);
    let loads: Vec<_> = output
        .lines()
        .filter(|line| line.trim_start().starts_with("LOAD"))
        .collect();
    if loads.len() != 3
        || !loads[0].contains("R E")
        || !loads[1].contains("R ")
        || !loads[2].contains("RW")
    {
        return Err(ConduitosError::refusal(
            "invalid-load-segments",
            format!("expected exact R+X/R/R+W LOAD segments, found {loads:?}"),
        ));
    }
    let sections = super::profile::command(
        "readelf",
        &["-SW", kernel],
        &paths.root,
        "readelf-unavailable",
    )?;
    if !String::from_utf8_lossy(&sections.stdout).contains(".requests") {
        return Err(ConduitosError::refusal(
            "missing-limine-requests",
            ".requests was not retained",
        ));
    }
    Ok(())
}

fn write_json(path: &std::path::Path, value: &BuildRecord) -> Result<(), ConduitosError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| ConduitosError::refusal("build-record-failed", error.to_string()))?;
    fs::write(path, bytes)
        .map_err(|error| ConduitosError::refusal("build-record-failed", error.to_string()))
}

fn dry_record(arch: ConduitosArch, paths: &Paths) -> Result<BuildRecord, ConduitosError> {
    let rust_target = match arch {
        ConduitosArch::X86_64 => "x86_64-unknown-none",
        ConduitosArch::Aarch64 => "aarch64-unknown-none",
        _ => {
            return Err(ConduitosError::refusal(
                "unsupported-build-architecture",
                arch.as_str(),
            ));
        }
    };
    Ok(BuildRecord {
        schema: "conduit.conduitos.build/v1",
        base_commit: git_head(&paths.root)?,
        architecture: arch.as_str(),
        rust_target,
        limine_crate: "0.5.0",
        elf_sha256: "dry-run".into(),
    })
}
