use std::{fs, path::Path, process::Command};

use conduit_host_conduitos_fabrication::ConduitOsProductArtifact;
use conduit_host_fabrication::{build_default_host_image, BuildInputs, BuildManifest, HostProfile};

use crate::cli::GlobalOpts;

use super::{
    aarch64_a0, armv6_rpi_b_plus_a0, ia32_a0, ia32_a2, loongarch64_a0,
    profile::{Paths, COMMON_BACKBONE_TARGETS},
    report::{git_head, sha256_file, ArtifactRole, BuildRecord},
    riscv64_a0, target_lowering, ConduitosArch, ConduitosError,
};

pub fn execute_architecture_proof(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    if arch == ConduitosArch::X86_64 {
        execute_embedded_profile(
            arch,
            include_str!("../../profiles/conduitos-native.profile.json"),
            ArtifactRole::ArchitectureProofAppliance,
            opts,
        )
    } else {
        execute_with_features(
            arch,
            opts,
            &[],
            None,
            ArtifactRole::ArchitectureProofAppliance,
            None,
        )
    }
}

pub(super) fn execute_profile(
    manifest: &BuildManifest,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    execute_profile_for_role(manifest, ArtifactRole::ProductHost, opts)
}

fn execute_profile_for_role(
    manifest: &BuildManifest,
    artifact_role: ArtifactRole,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    let arch = super::target_backend::select(&manifest.target)?.arch;
    let product_artifact = if artifact_role == ArtifactRole::ProductHost {
        Some(
            ConduitOsProductArtifact::for_target(&manifest.target).ok_or_else(|| {
                ConduitosError::refusal(
                    "product-artifact-resolution-failed",
                    format!("{} has no package-owned product artifact", manifest.target),
                )
            })?,
        )
    } else {
        None
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
        artifact_role,
        product_artifact,
    )
}

pub(super) fn execute_hotplug(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    execute_embedded_profile(
        arch,
        include_str!("../../proof/profiles/conduitos-hotplug-proof.profile.json"),
        ArtifactRole::ArchitectureProofAppliance,
        opts,
    )
}

pub(super) fn execute_proof(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    execute_embedded_profile(
        arch,
        include_str!("../../proof/profiles/conduitos-proof.profile.json"),
        ArtifactRole::ArchitectureProofAppliance,
        opts,
    )
}

fn execute_embedded_profile(
    arch: ConduitosArch,
    source: &str,
    artifact_role: ArtifactRole,
    opts: &GlobalOpts,
) -> Result<BuildRecord, ConduitosError> {
    let manifest = resolve_embedded_profile(arch, source)?;
    execute_profile_for_role(&manifest, artifact_role, opts)
}

pub(super) fn proof_manifest(arch: ConduitosArch) -> Result<BuildManifest, ConduitosError> {
    resolve_embedded_profile(
        arch,
        include_str!("../../proof/profiles/conduitos-proof.profile.json"),
    )
}

fn resolve_embedded_profile(
    arch: ConduitosArch,
    source: &str,
) -> Result<BuildManifest, ConduitosError> {
    let paths = Paths::new(arch)?;
    let profile: HostProfile = serde_json::from_str(source)
        .map_err(|error| ConduitosError::refusal("proof-profile-invalid", error.to_string()))?;
    if profile.target.architecture != arch.as_str() {
        return Err(ConduitosError::refusal(
            "profile-target-mismatch",
            format!(
                "{} profile cannot fabricate {}",
                profile.target.key(),
                arch.as_str()
            ),
        ));
    }
    let source_identity = git_head(&paths.root)?;
    let packages = conduit_workspace_fabrication::package_set();
    let (image, _) = build_default_host_image(
        profile,
        &conduit_workspace_fabrication::catalog(),
        &packages,
        &BuildInputs {
            source_identity,
            toolchain_available: true,
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
    artifact_role: ArtifactRole,
    product_artifact: Option<ConduitOsProductArtifact>,
) -> Result<BuildRecord, ConduitosError> {
    if arch == ConduitosArch::Ia32 && product_artifact.is_none() {
        return ia32_a2::execute(opts);
    }
    if arch == ConduitosArch::Aarch64 && fabrication.is_none() {
        return aarch64_a0::execute(opts);
    }
    if arch == ConduitosArch::Armv6 {
        return armv6_rpi_b_plus_a0::execute(Default::default(), opts);
    }
    if arch == ConduitosArch::Riscv64 && product_artifact.is_none() {
        return riscv64_a0::execute(opts);
    }
    if arch == ConduitosArch::Loongarch64 && product_artifact.is_none() {
        return loongarch64_a0::execute(opts);
    }
    let paths = Paths::new(arch)?;
    let (binary, target) = product_artifact.map_or_else(
        || match arch {
            ConduitosArch::X86_64 => ("conduitos", "x86_64-unknown-none"),
            _ => unreachable!("architecture proof appliance checked above"),
        },
        |artifact| (artifact.binary, artifact.rust_target),
    );
    if opts.dry_run {
        println!(
            "cargo build -p conduitos --bin {binary} --target {target} --release --features {}",
            features.join(",")
        );
        return dry_record(arch, &paths, artifact_role);
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
        .env("CONDUITOS_BUILD_ID", build_id)
        .env("CONDUITOS_IMAGE_ID", image_binding);
    if arch == ConduitosArch::Ia32 {
        let linker = ia32_a0::rust_lld(&paths.root)?;
        let script = paths
            .root
            .join("targets/conduitos/firmware/linker/ia32_product.ld");
        command.env(
            "RUSTFLAGS",
            format!(
                "-C relocation-model=static -C panic=abort -C linker={} -C link-arg=-T{} -C link-arg=--nostdlib -C link-arg=-no-pie -C link-arg=-z -C link-arg=max-page-size=0x1000",
                linker.display(),
                script.display()
            ),
        );
    } else if arch == ConduitosArch::Riscv64 {
        let linker = riscv64_a0::rust_lld(&paths.root)?;
        let script = paths
            .root
            .join("targets/conduitos/firmware/linker/riscv64_product.ld");
        command.env("RUSTFLAGS", format!("-C relocation-model=static -C panic=abort -C linker={} -C link-arg=-T{} -C link-arg=--nostdlib", linker.display(), script.display()));
    } else if arch == ConduitosArch::Loongarch64 {
        let linker = loongarch64_a0::rust_lld(&paths.root)?;
        let script = paths
            .root
            .join("targets/conduitos/firmware/linker/loongarch64_product.ld");
        command.env("RUSTFLAGS", format!("-C relocation-model=static -C panic=abort -C linker={} -C link-arg=-T{} -C link-arg=--nostdlib", linker.display(), script.display()));
    } else {
        command.env("RUSTFLAGS", "-C relocation-model=static -C panic=abort");
    }
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
    assert_elf(arch, &paths)?;
    let record = BuildRecord {
        schema: "conduit.conduitos.build/v2",
        artifact_role,
        base_commit,
        architecture: arch.as_str(),
        rust_target: target,
        limine_crate: if arch == ConduitosArch::Ia32 {
            "not-linked-multiboot1"
        } else {
            "0.5.0"
        },
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
        command.arg("check");
        if *target == armv6_rpi_b_plus_a0::TARGET {
            command
                .arg("-Zbuild-std=core,alloc")
                .env("RUSTC_BOOTSTRAP", "1");
        }
        command
            .args(["-p", "conduitos", "--lib", "--target", target])
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

fn assert_elf(arch: ConduitosArch, paths: &Paths) -> Result<(), ConduitosError> {
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
    let required_section = if arch == ConduitosArch::Ia32 {
        ".multiboot"
    } else {
        ".requests"
    };
    if !String::from_utf8_lossy(&sections.stdout).contains(required_section) {
        return Err(ConduitosError::refusal(
            "missing-boot-contract",
            format!("{required_section} was not retained"),
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

fn dry_record(
    arch: ConduitosArch,
    paths: &Paths,
    artifact_role: ArtifactRole,
) -> Result<BuildRecord, ConduitosError> {
    let rust_target = match arch {
        ConduitosArch::Ia32 => "i686-unknown-linux-gnu",
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
        schema: "conduit.conduitos.build/v2",
        artifact_role,
        base_commit: git_head(&paths.root)?,
        architecture: arch.as_str(),
        rust_target,
        limine_crate: if arch == ConduitosArch::Ia32 {
            "not-linked-multiboot1"
        } else {
            "0.5.0"
        },
        elf_sha256: "dry-run".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aarch64_architecture_build_is_typed_as_a_proof_appliance() {
        let record = execute_architecture_proof(
            ConduitosArch::Aarch64,
            &GlobalOpts {
                dry_run: true,
                ..GlobalOpts::default()
            },
        )
        .unwrap();
        assert_eq!(
            record.artifact_role,
            ArtifactRole::ArchitectureProofAppliance
        );
    }
}
