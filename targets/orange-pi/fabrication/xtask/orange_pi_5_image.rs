use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    orange_pi_5_media::{legacy_boot_script, read_fat_file, write_fat_files},
    profile::Paths,
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};

pub const TARGET_ID: &str = "conduitos/aarch64/orange-pi-5-rk3588s";
const RUST_TARGET: &str = "aarch64-unknown-none";
const BINARY: &str = "conduitos-aarch64-orange-pi-5";
const IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const SECTOR_BYTES: u64 = 512;
const BOOTLOADER_START_SECTOR: u64 = 64;
const PARTITION_START_SECTOR: u32 = 32_768;
const PARTITION_SECTORS: u32 = (IMAGE_BYTES / SECTOR_BYTES) as u32 - PARTITION_START_SECTOR;
const SOURCE_DATE_EPOCH: &str = "1786233600";
const U_BOOT_RELEASE: &str = "v2026.04";
const U_BOOT_ASSET: &str = "u-boot-orangepi-5-rk3588s.bin";
const U_BOOT_BYTES: u64 = 9_536_512;
const U_BOOT_SHA256: &str = "da275ba14e5480381353d72048059ed10ffd42b5de1653f4cb1f160a30c3d8bf";
const U_BOOT_URL: &str = "https://github.com/schneid-l/u-boot-rockchip/releases/download/v2026.04/u-boot-orangepi-5-rk3588s.bin";
const BOOT_COMMAND: &str =
    "fatload mmc ${devnum}:${distro_bootpart} ${kernel_addr_r} Image\nbooti ${kernel_addr_r} - -\n";

#[derive(Serialize)]
struct ImageRecord {
    schema: &'static str,
    proof_class: &'static str,
    base_commit: String,
    target_id: &'static str,
    architecture: &'static str,
    machine: &'static str,
    board: &'static str,
    os: Option<&'static str>,
    boot_mechanism: &'static str,
    bootloader_repository: &'static str,
    bootloader_release: &'static str,
    bootloader_asset: BootFileRecord,
    kernel: BootFileRecord,
    boot_script: BootFileRecord,
    image_sha256: String,
    image_bytes: u64,
    partition_scheme: &'static str,
    bootloader_start_sector: u64,
    partition_start_sector: u32,
    partition_sectors: u32,
    filesystem: &'static str,
    volume_label: &'static str,
    fabrication_package_id: &'static str,
    fabrication_package_revision: u32,
    output: &'static str,
    builder_adapter: &'static str,
    deployment_adapter: &'static str,
    source_identity: String,
    image_id: String,
    artifact: ArtifactRecord,
    boot_claimed: bool,
    physical_proof_claimed: bool,
}

#[derive(Serialize)]
struct ArtifactRecord {
    path: String,
    format: &'static str,
    bytes: u64,
    sha256: String,
}

#[derive(Serialize)]
struct BootFileRecord {
    path: String,
    bytes: u64,
    sha256: String,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let output = paths.target.join("orange-pi-5");
    let image = output.join("conduitos-orange-pi-5.img");
    if opts.dry_run {
        println!(
            "cargo build -p conduitos --bin {BINARY} --features aarch64-orange-pi-5 --target {RUST_TARGET} --release"
        );
        println!("fetch and verify {U_BOOT_ASSET} from {U_BOOT_RELEASE}");
        println!("assemble {}", image.display());
        return Ok(());
    }

    for tool in ["curl", "mkfs.vfat"] {
        require_tool(tool, &paths.root)?;
    }
    fs::create_dir_all(&output).map_err(|error| refusal("image-output-unavailable", error))?;
    let base_commit = git_head(&paths.root)?;
    build_kernel(&paths, &base_commit, opts)?;
    let kernel = output.join("Image");
    elf_to_image(
        &paths
            .root
            .join(format!("target/{RUST_TARGET}/release/{BINARY}")),
        &kernel,
    )?;
    let bootloader = prepare_bootloader(&paths)?;
    let stage = output.join("boot-stage");
    recreate_directory(&stage)?;
    fs::copy(&kernel, stage.join("Image"))
        .map_err(|error| refusal("image-staging-failed", error))?;
    fs::write(stage.join("boot.cmd"), BOOT_COMMAND)
        .map_err(|error| refusal("image-staging-failed", error))?;
    fs::write(
        stage.join("boot.scr"),
        legacy_boot_script(BOOT_COMMAND.as_bytes())?,
    )
    .map_err(|error| refusal("image-staging-failed", error))?;
    set_fixed_timestamps(&stage)?;

    write_partitioned_image(&image, &bootloader)?;
    run(
        Command::new("mkfs.vfat")
            .args([
                "--invariant",
                "--offset=32768",
                "-F",
                "32",
                "-h",
                "32768",
                "-i",
                "434e4435",
                "-n",
                "CONDUIT5",
            ])
            .arg(&image),
        "fat-filesystem-creation-failed",
    )?;
    write_fat_files(
        &image,
        &[
            (
                *b"IMAGE      ",
                fs::read(stage.join("Image"))
                    .map_err(|error| refusal("fat-file-copy-failed", error))?,
            ),
            (
                *b"BOOT    SCR",
                fs::read(stage.join("boot.scr"))
                    .map_err(|error| refusal("fat-file-copy-failed", error))?,
            ),
        ],
    )?;
    verify_image(&image, &bootloader, &stage, &output)?;

    let image_sha256 = sha256_file(&image)?;
    let record = ImageRecord {
        schema: "conduit.conduitos.orange-pi-5-image/v1",
        proof_class: "deterministic-image-artifact-only",
        base_commit: base_commit.clone(),
        target_id: TARGET_ID,
        architecture: "aarch64",
        machine: "rk3588s",
        board: "orange-pi-5",
        os: None,
        boot_mechanism: "rk3588s-bootrom-u-boot-booti-conduitos-image",
        bootloader_repository: "https://github.com/schneid-l/u-boot-rockchip",
        bootloader_release: U_BOOT_RELEASE,
        bootloader_asset: file_record(U_BOOT_ASSET, &bootloader)?,
        kernel: file_record("Image", &kernel)?,
        boot_script: file_record("boot.scr", &stage.join("boot.scr"))?,
        image_sha256: image_sha256.clone(),
        image_bytes: IMAGE_BYTES,
        partition_scheme: "mbr/rk3588-loader-at-lba64/single-fat32-lba32768",
        bootloader_start_sector: BOOTLOADER_START_SECTOR,
        partition_start_sector: PARTITION_START_SECTOR,
        partition_sectors: PARTITION_SECTORS,
        filesystem: "fat32",
        volume_label: "CONDUIT5",
        fabrication_package_id: "conduit-host-orange-pi@1",
        fabrication_package_revision: 1,
        output: "sd-image",
        builder_adapter: "conduit-host-orange-pi/build-conduitos-sd-image@1",
        deployment_adapter: "conduit-host-orange-pi/flash-removable-media@1",
        source_identity: format!("git:{base_commit}"),
        image_id: format!("conduitos-image/{base_commit}/orange-pi-5-rk3588s/v1"),
        artifact: ArtifactRecord {
            path: "conduitos-orange-pi-5.img".into(),
            format: "mbr-rk3588-fat32-sd-image",
            bytes: IMAGE_BYTES,
            sha256: format!("sha256:{image_sha256}"),
        },
        boot_claimed: false,
        physical_proof_claimed: false,
    };
    fs::write(
        output.join("orange-pi-5-image.json"),
        serde_json::to_vec_pretty(&record)
            .map_err(|error| refusal("image-record-failed", error))?,
    )
    .map_err(|error| refusal("image-record-failed", error))?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&record)
                .map_err(|error| refusal("image-record-failed", error))?
        );
    } else if !opts.quiet {
        println!("ConduitOS Orange Pi 5 SD image: {}", image.display());
    }
    Ok(())
}

fn build_kernel(paths: &Paths, base_commit: &str, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let mut command = Command::new("cargo");
    command
        .current_dir(&paths.root)
        .args([
            "build",
            "-p",
            "conduitos",
            "--bin",
            BINARY,
            "--features",
            "aarch64-orange-pi-5",
            "--target",
            RUST_TARGET,
            "--release",
        ])
        .env(
            "RUSTFLAGS",
            "-C relocation-model=static -C panic=abort -C opt-level=z -C codegen-units=1",
        )
        .env(
            "CONDUITOS_BUILD_ID",
            format!("conduitos-build/{base_commit}/orange-pi-5-rk3588s/v1"),
        )
        .env(
            "CONDUITOS_IMAGE_ID",
            format!("conduitos-image/{base_commit}/orange-pi-5-rk3588s/v1"),
        );
    if opts.locked {
        command.arg("--locked");
    }
    run(&mut command, "compile-link-failed")
}

fn elf_to_image(elf: &Path, output: &Path) -> Result<(), ConduitosError> {
    let bytes = fs::read(elf).map_err(|error| refusal("kernel-artifact-unavailable", error))?;
    if bytes.len() < 64
        || &bytes[..4] != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || u16_at(&bytes, 18)? != 183
    {
        return Err(refusal(
            "kernel-artifact-invalid",
            "expected one little-endian ELF64 AArch64 kernel",
        ));
    }
    let header_offset = usize::try_from(u64_at(&bytes, 32)?)
        .map_err(|_| refusal("kernel-artifact-invalid", "program table overflows usize"))?;
    let header_size = usize::from(u16_at(&bytes, 54)?);
    let header_count = usize::from(u16_at(&bytes, 56)?);
    let mut image = Vec::new();
    for index in 0..header_count {
        let offset = header_offset
            .checked_add(index.checked_mul(header_size).ok_or_else(|| {
                refusal("kernel-artifact-invalid", "program table offset overflow")
            })?)
            .ok_or_else(|| refusal("kernel-artifact-invalid", "program table offset overflow"))?;
        if u32_at(&bytes, offset)? != 1 {
            continue;
        }
        let file_offset = usize::try_from(u64_at(&bytes, offset + 8)?)
            .map_err(|_| refusal("kernel-artifact-invalid", "segment offset overflows usize"))?;
        let address = u64_at(&bytes, offset + 24)?;
        let file_bytes = usize::try_from(u64_at(&bytes, offset + 32)?)
            .map_err(|_| refusal("kernel-artifact-invalid", "segment size overflows usize"))?;
        if file_bytes == 0 {
            continue;
        }
        let image_offset = usize::try_from(address.checked_sub(0x0020_0000).ok_or_else(|| {
            refusal(
                "kernel-artifact-invalid",
                "load segment precedes 0x00200000",
            )
        })?)
        .map_err(|_| refusal("kernel-artifact-invalid", "image offset overflows usize"))?;
        let source_end = file_offset
            .checked_add(file_bytes)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| refusal("kernel-artifact-invalid", "load segment exceeds ELF"))?;
        let image_end = image_offset
            .checked_add(file_bytes)
            .ok_or_else(|| refusal("kernel-artifact-invalid", "raw image size overflow"))?;
        image.resize(image.len().max(image_end), 0);
        image[image_offset..image_end].copy_from_slice(&bytes[file_offset..source_end]);
    }
    if image.len() < 64 || image[56..60] != *b"ARMd" {
        return Err(refusal(
            "kernel-artifact-invalid",
            "AArch64 Image header is absent or displaced",
        ));
    }
    let image_size = u64::try_from(image.len())
        .map_err(|_| refusal("kernel-artifact-invalid", "raw image size overflows u64"))?;
    image[16..24].copy_from_slice(&image_size.to_le_bytes());
    fs::write(output, image).map_err(|error| refusal("kernel-image-write-failed", error))
}

fn prepare_bootloader(paths: &Paths) -> Result<PathBuf, ConduitosError> {
    let vendor = paths
        .root
        .join("target/conduitos/vendor")
        .join(format!("u-boot-rockchip-{U_BOOT_RELEASE}"));
    fs::create_dir_all(&vendor).map_err(|error| refusal("bootloader-cache-unavailable", error))?;
    let path = vendor.join(U_BOOT_ASSET);
    if !path.exists() {
        run(
            Command::new("curl")
                .args([
                    "--fail",
                    "--location",
                    "--silent",
                    "--show-error",
                    "--remove-on-error",
                    "--output",
                ])
                .arg(&path)
                .arg(U_BOOT_URL),
            "bootloader-fetch-failed",
        )?;
    }
    let metadata = fs::metadata(&path).map_err(|error| refusal("bootloader-invalid", error))?;
    let digest = sha256_file(&path)?;
    if metadata.len() != U_BOOT_BYTES || digest != U_BOOT_SHA256 {
        return Err(refusal(
            "bootloader-invalid",
            format!(
                "{U_BOOT_ASSET}: bytes={} sha256={digest}; expected bytes={U_BOOT_BYTES} sha256={U_BOOT_SHA256}",
                metadata.len()
            ),
        ));
    }
    Ok(path)
}

fn write_partitioned_image(path: &Path, bootloader: &Path) -> Result<(), ConduitosError> {
    let mut file =
        File::create(path).map_err(|error| refusal("image-output-unavailable", error))?;
    file.set_len(IMAGE_BYTES)
        .map_err(|error| refusal("image-output-unavailable", error))?;
    let mut mbr = [0_u8; 512];
    mbr[440..444].copy_from_slice(&[0x43, 0x4e, 0x44, 0x35]);
    let partition = &mut mbr[446..462];
    partition[0] = 0x80;
    partition[1..4].copy_from_slice(&[0x20, 0x21, 0x00]);
    partition[4] = 0x0c;
    partition[5..8].copy_from_slice(&[0xfe, 0xff, 0xff]);
    partition[8..12].copy_from_slice(&PARTITION_START_SECTOR.to_le_bytes());
    partition[12..16].copy_from_slice(&PARTITION_SECTORS.to_le_bytes());
    mbr[510..512].copy_from_slice(&[0x55, 0xaa]);
    file.write_all(&mbr)
        .map_err(|error| refusal("image-output-unavailable", error))?;
    file.seek(SeekFrom::Start(BOOTLOADER_START_SECTOR * SECTOR_BYTES))
        .map_err(|error| refusal("image-output-unavailable", error))?;
    let bytes = fs::read(bootloader).map_err(|error| refusal("bootloader-invalid", error))?;
    file.write_all(&bytes)
        .map_err(|error| refusal("image-output-unavailable", error))
}

fn verify_image(
    image: &Path,
    bootloader: &Path,
    stage: &Path,
    output: &Path,
) -> Result<(), ConduitosError> {
    let bytes = fs::read(image).map_err(|error| refusal("image-verification-failed", error))?;
    let loader =
        fs::read(bootloader).map_err(|error| refusal("image-verification-failed", error))?;
    let loader_start = (BOOTLOADER_START_SECTOR * SECTOR_BYTES) as usize;
    if bytes.len() as u64 != IMAGE_BYTES
        || bytes[440..444] != [0x43, 0x4e, 0x44, 0x35]
        || bytes[446] != 0x80
        || bytes[450] != 0x0c
        || bytes[454..458] != PARTITION_START_SECTOR.to_le_bytes()
        || bytes[458..462] != PARTITION_SECTORS.to_le_bytes()
        || bytes[510..512] != [0x55, 0xaa]
        || bytes[loader_start..loader_start + loader.len()] != loader
    {
        return Err(refusal(
            "image-verification-failed",
            "MBR, RK3588 loader placement, partition bounds, or image size is not exact",
        ));
    }
    for (name, fat_name) in [("Image", *b"IMAGE      "), ("boot.scr", *b"BOOT    SCR")] {
        if read_fat_file(image, fat_name)?
            != fs::read(stage.join(name))
                .map_err(|error| refusal("image-verification-failed", error))?
        {
            return Err(refusal(
                "image-verification-failed",
                format!("{name} bytes differ after FAT extraction"),
            ));
        }
    }
    let _ = output;
    Ok(())
}

fn file_record(path: &str, source: &Path) -> Result<BootFileRecord, ConduitosError> {
    Ok(BootFileRecord {
        path: path.into(),
        bytes: fs::metadata(source)
            .map_err(|error| refusal("image-record-failed", error))?
            .len(),
        sha256: format!("sha256:{}", sha256_file(source)?),
    })
}

fn recreate_directory(path: &Path) -> Result<(), ConduitosError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| refusal("image-staging-failed", error))?;
    }
    fs::create_dir_all(path).map_err(|error| refusal("image-staging-failed", error))
}

fn set_fixed_timestamps(path: &Path) -> Result<(), ConduitosError> {
    run(
        Command::new("touch")
            .args(["-d", &format!("@{SOURCE_DATE_EPOCH}")])
            .arg(path.join("Image"))
            .arg(path.join("boot.cmd"))
            .arg(path.join("boot.scr")),
        "image-staging-failed",
    )
}

fn require_tool(tool: &str, root: &Path) -> Result<(), ConduitosError> {
    run(
        Command::new("which").arg(tool).current_dir(root),
        "image-tool-unavailable",
    )
}

fn run(command: &mut Command, reason: &'static str) -> Result<(), ConduitosError> {
    let status = command
        .status()
        .map_err(|error| refusal(reason, format!("cannot launch command: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal(reason, status))
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, ConduitosError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| refusal("kernel-artifact-invalid", "ELF field is truncated"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, ConduitosError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| refusal("kernel-artifact-invalid", "ELF field is truncated"))?;
    Ok(u32::from_le_bytes(value.try_into().expect("four bytes")))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, ConduitosError> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| refusal("kernel-artifact-invalid", "ELF field is truncated"))?;
    Ok(u64::from_le_bytes(value.try_into().expect("eight bytes")))
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
#[path = "orange_pi_5_image_tests.rs"]
mod tests;
