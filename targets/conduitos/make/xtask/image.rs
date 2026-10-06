use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::Path,
};

use crate::cli::GlobalOpts;

use super::{
    build,
    profile::{
        command, command_with_env, Paths, LIMINE_ARCHIVE_SHA256, LIMINE_ARCHIVE_URL, LIMINE_VERSION,
    },
    report::{sha256_file, ArtifactRole, ImageRecord},
    ConduitosArch, ConduitosError,
};

const EXPECTED_IMAGE_FILE_COUNT: usize = 7;
const CONDUITOS_GPT_DISK_GUID: &str = "434f4e44-5549-544f-5300-000000000001";

pub fn execute_architecture_proof(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<ImageRecord, ConduitosError> {
    let build = build::execute_architecture_proof(arch, opts)?;
    build
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    assemble_architecture_proof(arch, opts)
}

pub(super) fn execute_hotplug(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<ImageRecord, ConduitosError> {
    let build = build::execute_hotplug(arch, opts)?;
    build
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    assemble_architecture_proof(arch, opts)
}

pub(super) fn execute_ps2_input(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<ImageRecord, ConduitosError> {
    let build = build::execute_ps2_input(arch, opts)?;
    build
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    assemble_architecture_proof(arch, opts)
}

pub(super) fn execute_proof(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<ImageRecord, ConduitosError> {
    let build = build::execute_proof(arch, opts)?;
    build
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    assemble_architecture_proof(arch, opts)
}

pub(super) fn execute_usb_configuration(opts: &GlobalOpts) -> Result<ImageRecord, ConduitosError> {
    let build = build::execute_usb_configuration(opts)?;
    build
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    assemble_architecture_proof(ConduitosArch::X86_64, opts)
}

pub(super) fn execute_usb_endpoint(opts: &GlobalOpts) -> Result<ImageRecord, ConduitosError> {
    let build = build::execute_usb_endpoint(opts)?;
    build
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    assemble_architecture_proof(ConduitosArch::X86_64, opts)
}

pub(super) fn execute_usb_hid_endpoint(
    opts: &GlobalOpts,
    mouse: bool,
) -> Result<ImageRecord, ConduitosError> {
    let build = build::execute_usb_hid_endpoint(opts, mouse)?;
    build
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    assemble_architecture_proof(ConduitosArch::X86_64, opts)
}

pub(super) fn execute_emergency_halt(opts: &GlobalOpts) -> Result<ImageRecord, ConduitosError> {
    let build = build::execute_emergency_halt(opts)?;
    build
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    assemble_architecture_proof(ConduitosArch::X86_64, opts)
}

pub(super) fn assemble_architecture_proof(
    arch: ConduitosArch,
    opts: &GlobalOpts,
) -> Result<ImageRecord, ConduitosError> {
    assemble_with_role(
        arch,
        None,
        ArtifactRole::ArchitectureProofAppliance,
        None,
        opts,
    )
}

pub(super) fn assemble_product(
    arch: ConduitosArch,
    build_description: &[u8],
    opts: &GlobalOpts,
) -> Result<ImageRecord, ConduitosError> {
    assemble_with_role(
        arch,
        Some(build_description),
        ArtifactRole::ProductHost,
        None,
        opts,
    )
}

pub(super) struct ProtocolModules<'a> {
    pub kernel: &'a Path,
    pub output: &'a Path,
    pub source: &'a [u8],
    pub request: &'a [u8],
}

pub(super) fn assemble_protocol(
    modules: ProtocolModules<'_>,
    opts: &GlobalOpts,
) -> Result<ImageRecord, ConduitosError> {
    assemble_with_role(
        ConduitosArch::X86_64,
        None,
        ArtifactRole::ProductHost,
        Some(modules),
        opts,
    )
}

fn stage_protocol(paths: &Paths, modules: ProtocolModules<'_>) -> Result<(), ConduitosError> {
    let boot = paths.iso_root.join("boot");
    fs::write(boot.join("conduit-protocol-source.json"), modules.source)
        .and_then(|()| fs::write(boot.join("conduit-protocol-root.json"), modules.request))
        .map_err(|error| {
            ConduitosError::refusal("protocol-image-staging-failed", error.to_string())
        })?;
    let mut config = fs::OpenOptions::new()
        .append(true)
        .open(paths.iso_root.join("limine.conf"))
        .map_err(|error| {
            ConduitosError::refusal("protocol-image-staging-failed", error.to_string())
        })?;
    config.write_all(b"    module_path: boot():/boot/conduit-protocol-source.json\n    module_string: conduit.protocol/source@1\n    module_path: boot():/boot/conduit-protocol-root.json\n    module_string: conduit.protocol/root-request@1\n")
        .map_err(|error| ConduitosError::refusal("protocol-image-staging-failed", error.to_string()))
}

fn assemble_with_role(
    arch: ConduitosArch,
    build_description: Option<&[u8]>,
    artifact_role: ArtifactRole,
    protocol: Option<ProtocolModules<'_>>,
    opts: &GlobalOpts,
) -> Result<ImageRecord, ConduitosError> {
    let mut paths = Paths::new(arch)?;
    let module_count = if protocol.is_some() { 2 } else { 0 };
    if let Some(modules) = &protocol {
        paths.kernel = modules.kernel.to_owned();
        paths.target = modules.output.to_owned();
        paths.iso_root = modules.output.join("iso-root");
        paths.iso = modules.output.join("conduitos.iso");
    }
    if opts.dry_run {
        println!("fetch and verify pinned Limine {LIMINE_VERSION}");
        println!("assemble {}", paths.iso.display());
        return Ok(ImageRecord {
            schema: "conduit.conduitos.image/v2",
            artifact_role,
            architecture: arch.as_str(),
            limine_version: LIMINE_VERSION,
            limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
            iso_sha256: "dry-run".into(),
            file_count: EXPECTED_IMAGE_FILE_COUNT
                + usize::from(build_description.is_some())
                + module_count,
        });
    }
    prepare_limine(&paths)?;
    stage_image(&paths, arch)?;
    if let Some(description) = build_description {
        fs::write(paths.iso_root.join("boot/conduit-build.json"), description)
            .map_err(|error| ConduitosError::refusal("image-staging-failed", error.to_string()))?;
    }
    if let Some(modules) = protocol {
        stage_protocol(&paths, modules)?;
    }
    create_iso(&paths)?;
    let file_count = count_files(&paths.iso_root)?;
    let expected_file_count =
        EXPECTED_IMAGE_FILE_COUNT + usize::from(build_description.is_some()) + module_count;
    if file_count != expected_file_count {
        return Err(ConduitosError::refusal(
            "unexpected-image-content",
            format!("staged {file_count} files; expected exactly {expected_file_count}"),
        ));
    }
    let record = ImageRecord {
        schema: "conduit.conduitos.image/v2",
        artifact_role,
        architecture: arch.as_str(),
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        iso_sha256: sha256_file(&paths.iso)?,
        file_count,
    };
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| ConduitosError::refusal("image-record-failed", error.to_string()))?;
    fs::write(paths.target.join("image.json"), bytes)
        .map_err(|error| ConduitosError::refusal("image-record-failed", error.to_string()))?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS image: {}", paths.iso.display());
    }
    Ok(record)
}

pub(super) fn prepare_limine(paths: &Paths) -> Result<(), ConduitosError> {
    let vendor = paths
        .limine_archive
        .parent()
        .ok_or_else(|| ConduitosError::refusal("limine-path-invalid", "missing vendor parent"))?;
    fs::create_dir_all(vendor)
        .map_err(|error| ConduitosError::refusal("limine-path-invalid", error.to_string()))?;
    if !paths.limine_archive.exists() {
        let archive = paths.limine_archive.to_str().ok_or_else(|| {
            ConduitosError::refusal("limine-path-invalid", "non-UTF-8 archive path")
        })?;
        command(
            "curl",
            &[
                "--fail",
                "--location",
                "--remove-on-error",
                "--retry",
                "3",
                "--retry-all-errors",
                "--retry-delay",
                "1",
                "--output",
                archive,
                LIMINE_ARCHIVE_URL,
            ],
            &paths.root,
            "missing-limine-artifacts",
        )?;
    }
    let digest = sha256_file(&paths.limine_archive)?;
    if digest != LIMINE_ARCHIVE_SHA256 {
        return Err(ConduitosError::refusal(
            "unsupported-limine-revision",
            format!("archive digest {digest} does not match pinned {LIMINE_ARCHIVE_SHA256}"),
        ));
    }
    if !paths.limine.join("limine").exists() {
        if paths.limine.exists() {
            fs::remove_dir_all(&paths.limine).map_err(|error| {
                ConduitosError::refusal("limine-path-invalid", error.to_string())
            })?;
        }
        let archive = paths.limine_archive.to_str().unwrap();
        let vendor_text = vendor.to_str().unwrap();
        command(
            "tar",
            &["-xzf", archive, "-C", vendor_text],
            &paths.root,
            "missing-limine-artifacts",
        )?;
        let extracted = vendor.join("limine-binary");
        fs::rename(extracted, &paths.limine).map_err(|error| {
            ConduitosError::refusal("missing-limine-artifacts", error.to_string())
        })?;
        command("make", &[], &paths.limine, "missing-limine-toolchain")?;
    }
    Ok(())
}

fn stage_image(paths: &Paths, arch: ConduitosArch) -> Result<(), ConduitosError> {
    if paths.iso_root.exists() {
        fs::remove_dir_all(&paths.iso_root)
            .map_err(|error| ConduitosError::refusal("image-staging-failed", error.to_string()))?;
    }
    let boot = paths.iso_root.join("boot");
    let limine_boot = boot.join("limine");
    let efi_boot = paths.iso_root.join("EFI/BOOT");
    fs::create_dir_all(&limine_boot)
        .and_then(|_| fs::create_dir_all(&efi_boot))
        .map_err(|error| ConduitosError::refusal("image-staging-failed", error.to_string()))?;
    copy(&paths.kernel, &boot.join("conduitos"))?;
    let mut spore = vec![0xff; conduitos::spore_provision::REGION_BYTES];
    let magic = b"CONDUIT_SPORE_MEDIA@1\0";
    spore[..magic.len()].copy_from_slice(magic);
    spore[24..32].fill(0);
    fs::write(boot.join("conduit-spore.bin"), spore)
        .map_err(|error| ConduitosError::refusal("image-staging-failed", error.to_string()))?;
    let config = match arch {
        ConduitosArch::Aarch64 => "targets/conduitos/firmware/boot/limine-aarch64-a1.conf",
        ConduitosArch::Ia32 => "targets/conduitos/firmware/boot/limine-ia32-a1.conf",
        ConduitosArch::Riscv64 => "targets/conduitos/firmware/boot/limine-riscv64-a1.conf",
        ConduitosArch::Loongarch64 => "targets/conduitos/firmware/boot/limine-loongarch64-a1.conf",
        _ => "targets/conduitos/firmware/boot/limine.conf",
    };
    copy(
        &paths.root.join(config),
        &paths.iso_root.join("limine.conf"),
    )?;
    for name in [
        "limine-bios.sys",
        "limine-bios-cd.bin",
        "limine-uefi-cd.bin",
    ] {
        copy(&paths.limine.join(name), &limine_boot.join(name))?;
    }
    let efi_name = match arch {
        ConduitosArch::Aarch64 => "BOOTAA64.EFI",
        ConduitosArch::Ia32 => "BOOTIA32.EFI",
        ConduitosArch::Riscv64 => "BOOTRISCV64.EFI",
        ConduitosArch::Loongarch64 => "BOOTLOONGARCH64.EFI",
        _ => "BOOTX64.EFI",
    };
    copy(&paths.limine.join(efi_name), &efi_boot.join(efi_name))?;
    Ok(())
}

fn create_iso(paths: &Paths) -> Result<(), ConduitosError> {
    let iso_root = paths.iso_root.to_str().unwrap();
    let iso = paths.iso.to_str().unwrap();
    command_with_env(
        "xorriso",
        &[
            "-as",
            "mkisofs",
            "-b",
            "boot/limine/limine-bios-cd.bin",
            "-no-emul-boot",
            "-boot-load-size",
            "4",
            "-boot-info-table",
            "--efi-boot",
            "boot/limine/limine-uefi-cd.bin",
            "-efi-boot-part",
            "--efi-boot-image",
            "--protective-msdos-label",
            "--gpt_disk_guid",
            CONDUITOS_GPT_DISK_GUID,
            "--modification-date=2026080900000000",
            "--set_all_file_dates",
            "2026080900000000",
            iso_root,
            "-o",
            iso,
        ],
        &paths.root,
        "missing-image-toolchain",
        &[("SOURCE_DATE_EPOCH", "1786233600")],
    )?;
    command(
        paths.limine.join("limine").to_str().unwrap(),
        &["bios-install", iso],
        &paths.root,
        "limine-install-failed",
    )?;
    seal_mbr_identity(&paths.iso)?;
    Ok(())
}

fn seal_mbr_identity(iso: &Path) -> Result<(), ConduitosError> {
    const MBR_DISK_SIGNATURE_OFFSET: u64 = 440;
    const CONDUITOS_MBR_SIGNATURE: [u8; 4] = [0x43, 0x4e, 0x44, 0x54];
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(iso)
        .map_err(|error| ConduitosError::refusal("image-sealing-failed", error.to_string()))?;
    file.seek(SeekFrom::Start(MBR_DISK_SIGNATURE_OFFSET))
        .and_then(|_| file.write_all(&CONDUITOS_MBR_SIGNATURE))
        .map_err(|error| ConduitosError::refusal("image-sealing-failed", error.to_string()))
}

fn copy(source: &Path, destination: &Path) -> Result<(), ConduitosError> {
    fs::copy(source, destination).map(|_| ()).map_err(|error| {
        ConduitosError::refusal(
            "image-staging-failed",
            format!("{} > {}: {error}", source.display(), destination.display()),
        )
    })
}

fn count_files(root: &Path) -> Result<usize, ConduitosError> {
    fn visit(path: &Path, count: &mut usize) -> std::io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                visit(&entry.path(), count)?;
            } else {
                *count += 1;
            }
        }
        Ok(())
    }
    let mut count = 0;
    visit(root, &mut count)
        .map_err(|error| ConduitosError::refusal("image-staging-failed", error.to_string()))?;
    Ok(count)
}
