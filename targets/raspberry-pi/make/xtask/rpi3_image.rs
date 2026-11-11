//! Exact Pi 3B+ product build and firmware SD image; no physical claim at build.
use super::{
    armv6_rpi_b_plus_image as media, build as product_build,
    profile::Paths,
    report::{git_head, sha256_file, ArtifactRole, BuildRecord},
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use std::{fs, path::Path, process::Command};

const TARGET: &str = "conduitos/aarch64/raspberry-pi-3-model-b-plus-rev-1.3";
const BINARY: &str = "conduitos-aarch64-rpi3-product";
const CONFIG: &str = "# ConduitOS Raspberry Pi 3 Model B+ rev 1.3\narm_64bit=1\nkernel=kernel8.img\nkernel_address=0x80000\ndisable_commandline_tags=1\ndevice_tree=\nenable_uart=1\nuart_2ndstage=1\ninit_uart_clock=3000000\ngpu_mem=64\ndisable_splash=1\n";
const FILES: [&str; 6] = [
    "LICENCE.broadcom",
    "bootcode.bin",
    "fixup.dat",
    "start.elf",
    "config.txt",
    "kernel8.img",
];

pub(super) fn build(
    opts: &GlobalOpts,
    make: &Path,
    build_id: &str,
    image_id: &str,
    features: &[&str],
) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let elf = paths.target.join("conduitos-rpi3.elf");
    if !opts.dry_run {
        let mut command = Command::new("cargo");
        command.current_dir(&paths.root)
            .args(["build", "-Zbuild-std=core,alloc", "-p", "conduitos", "--bin", BINARY,
                "--features", &features.join(","), "--target", "aarch64-unknown-none", "--release"])
            .env("RUSTC_BOOTSTRAP", "1")
            .env("CARGO_PROFILE_RELEASE_STRIP", "debuginfo")
            .env("CONDUITOS_MAKE_RECORD", make)
            .env("CONDUITOS_BUILD_ID", build_id)
            .env("CONDUITOS_IMAGE_ID", image_id)
            .env("RUSTFLAGS", "-C relocation-model=static -C panic=abort -C target-cpu=cortex-a53 -C opt-level=3 -C codegen-units=1");
        if opts.locked {
            command.arg("--locked");
        }
        run(&mut command)?;
        fs::copy(paths.built_binary("aarch64-unknown-none", BINARY), &elf).map_err(io_error)?;
        let header = fs::read(&elf).map_err(io_error)?;
        if header.len() < 64
            || &header[..6] != b"\x7fELF\x02\x01"
            || u16::from_le_bytes([header[18], header[19]]) != 183
            || u64::from_le_bytes(header[24..32].try_into().unwrap()) != 0x80000
        {
            return Err(refusal("rpi3-elf-entry-invalid"));
        }
        run(Command::new("llvm-objcopy")
            .args(["-O", "binary"])
            .arg(&elf)
            .arg(paths.target.join("kernel8.img")))?;
    }
    Ok(BuildRecord {
        schema: "conduit.conduitos.build/v2",
        artifact_role: ArtifactRole::ProductHost,
        base_commit: git_head(&paths.root)?,
        architecture: "aarch64",
        rust_target: "aarch64-unknown-none",
        limine_crate: "none",
        elf_sha256: if opts.dry_run {
            "dry-run".into()
        } else {
            sha256_file(&elf)?
        },
    })
}

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let manifest = product_build::resolve_embedded_profile(
        ConduitosArch::Aarch64,
        include_str!("../../../conduitos/profiles/conduitos-rpi3-bplus.host.conduit"),
    )?;
    if manifest.target != TARGET {
        return Err(refusal("rpi3-make-target-mismatch"));
    }
    product_build::execute_profile(&manifest, opts)?;
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    if opts.dry_run {
        println!(
            "assemble {}",
            paths.target.join("conduitos-rpi-3b-plus.img").display()
        );
        return Ok(());
    }
    let vendor = media::prepare_firmware(&paths)?;
    let stage = paths.target.join("rpi-3b-plus-boot-stage");
    fs::create_dir_all(&stage).map_err(io_error)?;
    for name in &FILES[..4] {
        fs::copy(vendor.join(name), stage.join(name)).map_err(io_error)?;
    }
    fs::copy(paths.target.join("kernel8.img"), stage.join("kernel8.img")).map_err(io_error)?;
    fs::write(stage.join("config.txt"), CONFIG).map_err(io_error)?;
    media::set_fixed_timestamps_for(&stage, &FILES)?;
    let image = paths.target.join("conduitos-rpi-3b-plus.img");
    media::write_partitioned_image(&image)?;
    run(Command::new("mkfs.vfat")
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
        .arg(&image))?;
    let image_spec = format!("{}@@1048576", image.display());
    for name in FILES {
        run(Command::new("mcopy")
            .args(["-m", "-i", &image_spec])
            .arg(stage.join(name))
            .arg(format!("::{name}")))?;
    }
    media::verify_partition_layout(&image)?;
    let mut files = Vec::new();
    for name in FILES {
        let original = fs::read(stage.join(name)).map_err(io_error)?;
        let actual = Command::new("mtype")
            .args(["-i", &image_spec, &format!("::{name}")])
            .output()
            .map_err(io_error)?;
        if !actual.status.success() || actual.stdout != original {
            return Err(refusal("rpi3-fat-file-verification-failed"));
        }
        files.push(serde_json::json!({"path":name,"bytes":original.len(),"sha256":format!("sha256:{}",sha256_file(&stage.join(name))?)}));
    }
    let receipt = serde_json::json!({
        "schema":"conduit.conduitos.rpi3-image/v1", "proof_class":"deterministic-image-artifact-only",
        "base_commit":git_head(&paths.root)?, "architecture":"aarch64", "machine":"BCM2837B0/Cortex-A53",
        "board":"raspberry-pi-3-model-b-plus-rev-1.3", "target_id":TARGET,
        "boot_mechanism":"raspberry-pi-videocore-firmware-direct-kernel",
        "firmware_commit":media::FIRMWARE_COMMIT,
        "make_package_id":"conduit-host-raspberry-pi@1",
        "make_package_revision":conduit_host_raspberry_pi::MAKE_PACKAGE_REVISION,
        "image_id":manifest.image_id,"image_path":image,"image_sha256":sha256_file(&image)?,
        "image_bytes":fs::metadata(&image).map_err(io_error)?.len(),
        "partition_scheme":"mbr/single-fat32-lba","partition_start_sector":2048,
        "boot_files":files,"boot_claimed":false,"physical_proof_claimed":false
    });
    fs::write(
        paths.target.join("rpi-3b-plus-image.json"),
        serde_json::to_vec_pretty(&receipt).map_err(io_error)?,
    )
    .map_err(io_error)?;
    println!("ConduitOS Raspberry Pi 3B+ SD image: {}", image.display());
    Ok(())
}
fn run(command: &mut Command) -> Result<(), ConduitosError> {
    let status = command.status().map_err(io_error)?;
    if status.success() {
        Ok(())
    } else {
        Err(ConduitosError::refusal(
            "rpi3-command-failed",
            status.to_string(),
        ))
    }
}
fn io_error(error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal("rpi3-artifact-error", error.to_string())
}
fn refusal(reason: &'static str) -> ConduitosError {
    ConduitosError::refusal(reason, TARGET)
}
