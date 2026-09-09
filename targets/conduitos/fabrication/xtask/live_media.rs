//! Canonical ConduitOS product media and boot entrances.

use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use serde::Serialize;

use crate::{cli::GlobalOpts, workspace::workspace_root};

use super::{
    demo,
    profile::{LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    removable_media,
    report::{git_head, sha256_file},
    ConduitosError,
};

const IA32_IMAGE_PACKAGER: &str = "pinned-limine-hybrid-iso";
const IA32_BOOTLOADER: &str = "Limine";
const IA32_FIRMWARE_ENVIRONMENT: &str = "x86-bios";
const IA32_BIOS_BOOT_ENTRY: &str = "boot/limine/limine-bios-cd.bin";

#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LiveHost {
    #[value(name = "x86_64", alias = "x86-64")]
    #[default]
    X86_64,
    Ia32,
    Aarch64,
    Riscv64,
    Loongarch64,
}

#[derive(Clone, Copy, Serialize)]
pub(super) struct LiveMediaRow {
    pub host: &'static str,
    pub profile: &'static str,
    pub artifact: &'static str,
    pub format: &'static str,
    pub graphical: bool,
    pub emulator: &'static str,
}

impl LiveHost {
    const ALL: [Self; 5] = [
        Self::X86_64,
        Self::Ia32,
        Self::Aarch64,
        Self::Riscv64,
        Self::Loongarch64,
    ];

    pub(super) const fn row(self) -> LiveMediaRow {
        match self {
            Self::X86_64 => LiveMediaRow {
                host: "conduitos/x86_64/pc",
                profile: "conduitos-native.profile.json",
                artifact: "conduitos-x86_64.iso",
                format: "hybrid BIOS/UEFI ISO",
                graphical: true,
                emulator: "QEMU q35",
            },
            Self::Ia32 => LiveMediaRow {
                host: "conduitos/ia32/pc",
                profile: "conduitos-ia32-headless.profile.json",
                artifact: "conduitos-ia32.iso",
                format: "hybrid BIOS/UEFI ISO",
                graphical: false,
                emulator: "QEMU pc",
            },
            Self::Aarch64 => LiveMediaRow {
                host: "conduitos/aarch64/virt",
                profile: "conduitos-aarch64-headless.profile.json",
                artifact: "conduitos-aarch64.iso",
                format: "UEFI ISO",
                graphical: false,
                emulator: "QEMU virt",
            },
            Self::Riscv64 => LiveMediaRow {
                host: "conduitos/riscv64/virt",
                profile: "conduitos-riscv64-headless.profile.json",
                artifact: "conduitos-riscv64.iso",
                format: "UEFI ISO",
                graphical: false,
                emulator: "QEMU virt",
            },
            Self::Loongarch64 => LiveMediaRow {
                host: "conduitos/loongarch64/virt",
                profile: "conduitos-loongarch64-headless.profile.json",
                artifact: "conduitos-loongarch64.iso",
                format: "UEFI ISO",
                graphical: false,
                emulator: "QEMU virt",
            },
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64-pc",
            Self::Ia32 => "ia32-pc",
            Self::Aarch64 => "aarch64-virt",
            Self::Riscv64 => "riscv64-virt",
            Self::Loongarch64 => "loongarch64-virt",
        }
    }
}

pub(super) fn build(host: LiveHost, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let root = root()?;
    let row = host.row();
    let profile = root.join("targets/conduitos/profiles").join(row.profile);
    let output = output(&root, host);
    let receipt = crate::commands::host::build_conduitos_live(&profile, &output, opts)
        .map_err(|error| ConduitosError::refusal("live-media-build-failed", error.to_string()))?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string(&receipt).map_err(|error| {
                ConduitosError::refusal("live-media-receipt-invalid", error.to_string())
            })?
        );
    } else if !opts.quiet {
        println!(
            "ConduitOS live media: {}",
            output.join(row.artifact).display()
        );
        println!("Host type: {}", row.host);
        println!("manifest: {}", output.join("build-manifest.json").display());
    }
    Ok(())
}

pub(super) fn boot(host: LiveHost, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let root = root()?;
    let row = host.row();
    let output = output(&root, host);
    if opts.dry_run && host == LiveHost::X86_64 {
        return demo::boot_visible_image(&output.join(row.artifact), None, opts);
    }
    let manifest = crate::commands::host::host_target::verify_target(&output)
        .map_err(|error| ConduitosError::refusal("live-media-invalid", error.to_string()))?;
    let image = output.join(&manifest.image.file);
    if host == LiveHost::X86_64 {
        demo::boot_visible_image(&image, Some(&manifest.image.sha256), opts)
    } else {
        crate::commands::host::host_target::boot_target(&output, &manifest, opts)
            .map_err(|error| ConduitosError::refusal("live-media-boot-failed", error.to_string()))
    }
}

pub(super) fn prove_ia32_legacy_bios(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let root = root()?;
    let output = output(&root, LiveHost::Ia32);
    let manifest = crate::commands::host::host_target::verify_target(&output)
        .map_err(|error| ConduitosError::refusal("live-media-invalid", error.to_string()))?;
    super::ia32_product_boot::boot_legacy_bios(
        &output.join(&manifest.image.file),
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )
}

#[derive(Serialize)]
struct Ia32FlashRecord {
    schema: &'static str,
    base_commit: String,
    architecture: &'static str,
    host: &'static str,
    profile_id: String,
    build_id: String,
    image_id: String,
    image_sha256: String,
    image_bytes: u64,
    image_packager: &'static str,
    bootloader: &'static str,
    bootloader_version: String,
    bootloader_archive_sha256: String,
    firmware_environment: &'static str,
    firmware_boot_entry: &'static str,
    carrier: &'static str,
    device: String,
    removable: bool,
    device_bytes: u64,
    write_completed: bool,
    byte_verification_completed: bool,
    physical_boot_claimed: bool,
}

pub(super) fn flash_ia32(
    requested: &Path,
    confirmed: &Path,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let device = removable_media::inspect_confirmed(requested, confirmed)?;
    if opts.dry_run {
        println!(
            "build canonical IA-32 live media; erase, write, and verify {}",
            device.path.display()
        );
        return Ok(());
    }

    build(LiveHost::Ia32, opts)?;
    let root = root()?;
    let output = output(&root, LiveHost::Ia32);
    let manifest = crate::commands::host::host_target::verify_target(&output)
        .map_err(|error| ConduitosError::refusal("live-media-invalid", error.to_string()))?;
    if manifest.boot_assets.packager != IA32_IMAGE_PACKAGER
        || manifest.boot_assets.limine_version != LIMINE_VERSION
        || manifest.boot_assets.limine_archive_sha256 != LIMINE_ARCHIVE_SHA256
        || manifest.boot_assets.architecture != "ia32"
        || manifest.boot_assets.machine != "pc"
    {
        return Err(ConduitosError::refusal(
            "ia32-bios-boot-assets-invalid",
            "the verified IA-32 IMAGE does not carry the exact pinned hybrid Limine identity",
        ));
    }
    let image = output.join(&manifest.image.file);
    let image_bytes = fs::metadata(&image)
        .map_err(|error| ConduitosError::refusal("flash-image-unavailable", error.to_string()))?
        .len();
    removable_media::write_and_verify(&image, image_bytes, &device)?;

    let record = Ia32FlashRecord {
        schema: "conduit.conduitos.ia32-live-flash/v2",
        base_commit: git_head(&root)?,
        architecture: "ia32",
        host: LiveHost::Ia32.row().host,
        profile_id: manifest.profile_id,
        build_id: manifest.build_id,
        image_id: manifest.image_id,
        image_sha256: sha256_file(&image)?,
        image_bytes,
        image_packager: IA32_IMAGE_PACKAGER,
        bootloader: IA32_BOOTLOADER,
        bootloader_version: manifest.boot_assets.limine_version,
        bootloader_archive_sha256: manifest.boot_assets.limine_archive_sha256,
        firmware_environment: IA32_FIRMWARE_ENVIRONMENT,
        firmware_boot_entry: IA32_BIOS_BOOT_ENTRY,
        carrier: "removable-whole-device",
        device: device.path.display().to_string(),
        removable: true,
        device_bytes: device.bytes,
        write_completed: true,
        byte_verification_completed: true,
        physical_boot_claimed: false,
    };
    let encoded = serde_json::to_vec_pretty(&record)
        .map_err(|error| ConduitosError::refusal("flash-record-failed", error.to_string()))?;
    fs::write(output.join("ia32-live-flash.json"), &encoded)
        .map_err(|error| ConduitosError::refusal("flash-record-failed", error.to_string()))?;
    if opts.json {
        println!(
            "{}",
            String::from_utf8(encoded).map_err(|error| {
                ConduitosError::refusal("flash-record-failed", error.to_string())
            })?
        );
    } else if !opts.quiet {
        println!(
            "Wrote and byte-verified {} on {}",
            image.display(),
            device.path.display()
        );
    }
    Ok(())
}

pub(super) fn matrix(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    #[derive(Serialize)]
    struct Matrix {
        schema: &'static str,
        output_root: &'static str,
        live_media: Vec<LiveMediaRow>,
        capability_gaps: [&'static str; 3],
    }
    let matrix = Matrix {
        schema: "conduit.conduitos/live-media-matrix@1",
        output_root: "target/conduitos/live/<host>",
        live_media: LiveHost::ALL.into_iter().map(LiveHost::row).collect(),
        capability_gaps: [
            "armv6 Raspberry Pi images remain proof-appliance media, not normal product Hosts",
            "Orange Pi 5 media remains deterministic artifact-only, without accepted product boot",
            "only x86_64 currently has the native graphical shell and local input",
        ],
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&matrix).map_err(|error| {
            ConduitosError::refusal("live-media-matrix-invalid", error.to_string())
        })?
    );
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-live-media-matrix",
            "the matrix is static checked product truth and does not need a dry run",
        ));
    }
    Ok(())
}

fn root() -> Result<PathBuf, ConduitosError> {
    workspace_root().map_err(|error| ConduitosError::refusal("workspace-unavailable", error))
}

fn output(root: &Path, host: LiveHost) -> PathBuf {
    root.join("target/conduitos/live").join(host.slug())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_names_every_current_product_host_and_no_proof_appliance() {
        let rows = LiveHost::ALL.map(LiveHost::row);
        assert_eq!(rows.len(), 5);
        assert!(rows
            .iter()
            .any(|row| row.host == "conduitos/x86_64/pc" && row.graphical));
        assert!(rows.iter().all(|row| row.artifact.ends_with(".iso")));
        assert!(!rows
            .iter()
            .any(|row| row.host.contains("armv6") || row.host.contains("orange")));
    }

    #[test]
    fn canonical_output_is_host_specific_and_predictable() {
        let root = Path::new("/repo");
        assert_eq!(
            output(root, LiveHost::X86_64),
            Path::new("/repo/target/conduitos/live/x86_64-pc")
        );
        assert_eq!(LiveHost::Riscv64.row().artifact, "conduitos-riscv64.iso");
    }
}
