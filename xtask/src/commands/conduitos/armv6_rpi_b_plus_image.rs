use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    armv6_rpi_b_plus_a0,
    armv6_rpi_board::Armv6RpiBoard,
    profile::Paths,
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};

const FIRMWARE_COMMIT: &str = "06df1d1a5cc34cc32a4d748239dc1523711fae8b";
const IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const SECTOR_BYTES: u64 = 512;
const PARTITION_START_SECTOR: u32 = 2048;
const PARTITION_SECTORS: u32 = (IMAGE_BYTES / SECTOR_BYTES) as u32 - PARTITION_START_SECTOR;
const FAT_OFFSET_BYTES: u64 = PARTITION_START_SECTOR as u64 * SECTOR_BYTES;
const SOURCE_DATE_EPOCH: &str = "1786233600";
const CONFIG_BODY: &str = "arm_64bit=0\nkernel=kernel.img\ndisable_commandline_tags=1\ndevice_tree=\nenable_uart=1\ngpu_mem=16\ndisable_splash=1\n";

const ASSETS: [FirmwareAsset; 4] = [
    FirmwareAsset::new(
        "LICENCE.broadcom",
        "c7283ff51f863d93a275c66e3b4cb08021a5dd4d8c1e7acc47d872fbe52d3d6b",
        1594,
    ),
    FirmwareAsset::new(
        "bootcode.bin",
        "4245ec23b58158dc3e55f3e065eb4ecc0c0705d76c8db506e9f824941feee32e",
        52624,
    ),
    FirmwareAsset::new(
        "fixup.dat",
        "651632e140c932cba9ed64bb7c8871ec927202b000968009a055171f09ee96ff",
        7381,
    ),
    FirmwareAsset::new(
        "start.elf",
        "6a79bc2f28a51be84e1be03d508c1c4aa205e287346ebf30df7a0961054993e8",
        3_022_336,
    ),
];

#[derive(Clone, Copy)]
struct FirmwareAsset {
    name: &'static str,
    sha256: &'static str,
    bytes: u64,
}

impl FirmwareAsset {
    const fn new(name: &'static str, sha256: &'static str, bytes: u64) -> Self {
        Self {
            name,
            sha256,
            bytes,
        }
    }
}

#[derive(Serialize)]
struct ImageRecord {
    schema: &'static str,
    proof_class: &'static str,
    base_commit: String,
    architecture: &'static str,
    machine: &'static str,
    board: &'static str,
    boot_mechanism: &'static str,
    firmware_repository: &'static str,
    firmware_commit: &'static str,
    firmware_assets: Vec<FirmwareAssetRecord>,
    image_path: String,
    image_sha256: String,
    image_bytes: u64,
    partition_scheme: &'static str,
    partition_start_sector: u32,
    partition_sectors: u32,
    filesystem: &'static str,
    volume_label: &'static str,
    files: [&'static str; 6],
    kernel_image_sha256: String,
    boot_claimed: bool,
    physical_proof_claimed: bool,
}

#[derive(Serialize)]
struct FirmwareAssetRecord {
    name: &'static str,
    sha256: &'static str,
    bytes: u64,
}

pub fn execute(board: Armv6RpiBoard, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Armv6)?;
    if opts.dry_run {
        armv6_rpi_b_plus_a0::execute(board, opts)?;
        println!("fetch and verify Raspberry Pi firmware {FIRMWARE_COMMIT}");
        println!("assemble {}", image_path(&paths, board).display());
        return Ok(());
    }
    armv6_rpi_b_plus_a0::execute(board, opts)?;
    require_tool("mkfs.vfat", &paths.root)?;
    require_tool("mcopy", &paths.root)?;
    fs::create_dir_all(&paths.target)
        .map_err(|error| refusal("image-output-unavailable", error))?;
    let vendor = prepare_firmware(&paths)?;
    let stage = paths
        .target
        .join(format!("{}-boot-stage", board.artifact_slug()));
    recreate_directory(&stage, "image-staging-failed")?;
    for asset in ASSETS {
        fs::copy(vendor.join(asset.name), stage.join(asset.name))
            .map_err(|error| refusal("image-staging-failed", error))?;
    }
    fs::copy(paths.target.join("kernel.img"), stage.join("kernel.img"))
        .map_err(|error| refusal("image-staging-failed", error))?;
    fs::write(
        stage.join("config.txt"),
        format!("{}{CONFIG_BODY}", board.config_heading()),
    )
    .map_err(|error| refusal("image-staging-failed", error))?;
    set_fixed_timestamps(&stage)?;

    let image = image_path(&paths, board);
    write_partitioned_image(&image)?;
    run(
        Command::new("mkfs.vfat")
            .args([
                "--invariant",
                "--offset=2048",
                "-F",
                "32",
                "-h",
                "2048",
                "-i",
                "434e4454",
                "-n",
                "CONDUITOS",
            ])
            .arg(&image),
        "fat-filesystem-creation-failed",
    )?;
    for name in image_files() {
        run(
            Command::new("mcopy")
                .env("MTOOLS_SKIP_CHECK", "1")
                .args(["-m", "-i"])
                .arg(mtools_image(&image)?)
                .arg(stage.join(name))
                .arg(format!("::{name}")),
            "fat-file-copy-failed",
        )?;
    }
    verify_image(&image, &stage, &paths.target)?;
    let record = ImageRecord {
        schema: "conduit.conduitos.armv6-rpi-image/v1",
        proof_class: "deterministic-image-artifact-only",
        base_commit: git_head(&paths.root)?,
        architecture: "armv6",
        machine: "BCM2835/ARM1176JZF-S",
        board: board.id(),
        boot_mechanism: "raspberry-pi-videocore-firmware-direct-kernel",
        firmware_repository: "https://github.com/raspberrypi/firmware",
        firmware_commit: FIRMWARE_COMMIT,
        firmware_assets: ASSETS
            .into_iter()
            .map(|asset| FirmwareAssetRecord {
                name: asset.name,
                sha256: asset.sha256,
                bytes: asset.bytes,
            })
            .collect(),
        image_path: image.display().to_string(),
        image_sha256: sha256_file(&image)?,
        image_bytes: IMAGE_BYTES,
        partition_scheme: "mbr/single-fat32-lba",
        partition_start_sector: PARTITION_START_SECTOR,
        partition_sectors: PARTITION_SECTORS,
        filesystem: "fat32",
        volume_label: "CONDUITOS",
        files: image_files(),
        kernel_image_sha256: sha256_file(&paths.target.join("kernel.img"))?,
        boot_claimed: false,
        physical_proof_claimed: false,
    };
    fs::write(
        paths
            .target
            .join(format!("{}-image.json", board.artifact_slug())),
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
        println!("ConduitOS Raspberry Pi SD image: {}", image.display());
    }
    Ok(())
}

fn prepare_firmware(paths: &Paths) -> Result<PathBuf, ConduitosError> {
    let vendor = paths
        .root
        .join("target/conduitos/vendor")
        .join(format!("raspberrypi-firmware-{FIRMWARE_COMMIT}"));
    fs::create_dir_all(&vendor).map_err(|error| refusal("firmware-cache-unavailable", error))?;
    for asset in ASSETS {
        let path = vendor.join(asset.name);
        if !path.exists() {
            let url = format!(
                "https://raw.githubusercontent.com/raspberrypi/firmware/{FIRMWARE_COMMIT}/boot/{}",
                asset.name
            );
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
                    .arg(url),
                "firmware-fetch-failed",
            )?;
        }
        let metadata = fs::metadata(&path).map_err(|error| refusal("firmware-invalid", error))?;
        let digest = sha256_file(&path)?;
        if metadata.len() != asset.bytes || digest != asset.sha256 {
            return Err(refusal(
                "firmware-invalid",
                format!(
                    "{}: bytes={} sha256={digest}; expected bytes={} sha256={}",
                    asset.name,
                    metadata.len(),
                    asset.bytes,
                    asset.sha256
                ),
            ));
        }
    }
    Ok(vendor)
}

fn write_partitioned_image(path: &Path) -> Result<(), ConduitosError> {
    let mut file =
        File::create(path).map_err(|error| refusal("image-output-unavailable", error))?;
    file.set_len(IMAGE_BYTES)
        .map_err(|error| refusal("image-output-unavailable", error))?;
    let mut mbr = [0_u8; 512];
    mbr[440..444].copy_from_slice(&[0x43, 0x4e, 0x44, 0x54]);
    let partition = &mut mbr[446..462];
    partition[1..4].copy_from_slice(&[0x20, 0x21, 0x00]);
    partition[4] = 0x0c;
    partition[5..8].copy_from_slice(&[0xfe, 0xff, 0xff]);
    partition[8..12].copy_from_slice(&PARTITION_START_SECTOR.to_le_bytes());
    partition[12..16].copy_from_slice(&PARTITION_SECTORS.to_le_bytes());
    mbr[510..512].copy_from_slice(&[0x55, 0xaa]);
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.write_all(&mbr))
        .map_err(|error| refusal("image-output-unavailable", error))
}

fn verify_image(image: &Path, stage: &Path, target: &Path) -> Result<(), ConduitosError> {
    let bytes = fs::read(image).map_err(|error| refusal("image-verification-failed", error))?;
    if bytes.len() as u64 != IMAGE_BYTES
        || bytes[440..444] != [0x43, 0x4e, 0x44, 0x54]
        || bytes[450] != 0x0c
        || bytes[454..458] != PARTITION_START_SECTOR.to_le_bytes()
        || bytes[458..462] != PARTITION_SECTORS.to_le_bytes()
        || bytes[510..512] != [0x55, 0xaa]
    {
        return Err(refusal(
            "image-verification-failed",
            "MBR identity, partition bounds, or image size is not exact",
        ));
    }
    let extracted = target.join("rpi-image-verify");
    recreate_directory(&extracted, "image-verification-failed")?;
    for name in image_files() {
        run(
            Command::new("mcopy")
                .env("MTOOLS_SKIP_CHECK", "1")
                .args(["-i"])
                .arg(mtools_image(image)?)
                .arg(format!("::{name}"))
                .arg(extracted.join(name)),
            "image-verification-failed",
        )?;
        let expected = fs::read(stage.join(name))
            .map_err(|error| refusal("image-verification-failed", error))?;
        let actual = fs::read(extracted.join(name))
            .map_err(|error| refusal("image-verification-failed", error))?;
        if actual != expected {
            return Err(refusal(
                "image-verification-failed",
                format!("{name} bytes differ after FAT extraction"),
            ));
        }
    }
    fs::remove_dir_all(extracted).map_err(|error| refusal("image-verification-failed", error))
}

fn recreate_directory(path: &Path, reason: &'static str) -> Result<(), ConduitosError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| refusal(reason, error))?;
    }
    fs::create_dir_all(path).map_err(|error| refusal(reason, error))
}

fn set_fixed_timestamps(stage: &Path) -> Result<(), ConduitosError> {
    let mut command = Command::new("touch");
    command.args(["--date", &format!("@{SOURCE_DATE_EPOCH}")]);
    for name in image_files() {
        command.arg(stage.join(name));
    }
    run(&mut command, "image-staging-failed")
}

const fn image_files() -> [&'static str; 6] {
    [
        "LICENCE.broadcom",
        "bootcode.bin",
        "config.txt",
        "fixup.dat",
        "kernel.img",
        "start.elf",
    ]
}

pub(super) fn image_path(paths: &Paths, board: Armv6RpiBoard) -> PathBuf {
    paths
        .target
        .join(format!("conduitos-{}.img", board.artifact_slug()))
}

fn mtools_image(image: &Path) -> Result<String, ConduitosError> {
    let path = image
        .to_str()
        .ok_or_else(|| refusal("image-path-invalid", "non-UTF-8 image path"))?;
    Ok(format!("{path}@@{FAT_OFFSET_BYTES}"))
}

fn require_tool(program: &str, root: &Path) -> Result<(), ConduitosError> {
    let output = Command::new(program)
        .arg("--help")
        .current_dir(root)
        .output()
        .map_err(|error| refusal("missing-rpi-image-toolchain", error))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(refusal(
            "missing-rpi-image-toolchain",
            format!("{program}: {}", output.status),
        ))
    }
}

fn run(command: &mut Command, reason: &'static str) -> Result<(), ConduitosError> {
    let description = format!("{command:?}");
    let status = command
        .status()
        .map_err(|error| refusal(reason, format!("{description}: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal(reason, format!("{description}: {status}")))
    }
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_geometry_is_finite_and_partition_bounded() {
        assert_eq!(PARTITION_START_SECTOR, 2048);
        assert_eq!(PARTITION_SECTORS, 129_024);
        assert_eq!(FAT_OFFSET_BYTES, 1024 * 1024);
        assert_eq!(
            PARTITION_START_SECTOR as u64 + PARTITION_SECTORS as u64,
            IMAGE_BYTES / SECTOR_BYTES
        );
    }

    #[test]
    fn configuration_selects_exact_32_bit_direct_kernel() {
        assert!(CONFIG_BODY.contains("arm_64bit=0\n"));
        assert!(CONFIG_BODY.contains("kernel=kernel.img\n"));
        assert!(CONFIG_BODY.contains("device_tree=\n"));
        assert!(!CONFIG_BODY.contains("kernel_old=1"));
        assert!(!CONFIG_BODY.to_ascii_lowercase().contains("limine"));
        assert_ne!(Armv6RpiBoard::BPlusV1_2.id(), Armv6RpiBoard::ZeroV1.id());
    }
}
