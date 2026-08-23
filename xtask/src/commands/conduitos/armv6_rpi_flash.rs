use std::{
    fs,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    armv6_rpi_b_plus_image,
    armv6_rpi_board::Armv6RpiBoard,
    profile::Paths,
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};

#[derive(Serialize)]
struct FlashRecord {
    schema: &'static str,
    base_commit: String,
    architecture: &'static str,
    board: &'static str,
    device: String,
    removable: bool,
    device_bytes: u64,
    image_sha256: String,
    image_bytes: u64,
    write_completed: bool,
    byte_verification_completed: bool,
    physical_boot_claimed: bool,
}

pub fn execute(
    board: Armv6RpiBoard,
    requested: &Path,
    confirmed: &Path,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    if requested != confirmed {
        return Err(refusal(
            "flash-device-confirmation-mismatch",
            format!(
                "--device {} differs from --confirm-device {}",
                requested.display(),
                confirmed.display()
            ),
        ));
    }
    let device = inspect_device(requested)?;
    if opts.dry_run {
        println!(
            "build ARMv6 Raspberry Pi image; erase, write, and verify {}",
            device.path.display()
        );
        return Ok(());
    }
    armv6_rpi_b_plus_image::execute(board, opts)?;
    let paths = Paths::new(ConduitosArch::Armv6)?;
    let image = armv6_rpi_b_plus_image::image_path(&paths, board);
    let image_bytes = fs::metadata(&image)
        .map_err(|error| refusal("flash-image-unavailable", error))?
        .len();
    if device.bytes < image_bytes {
        return Err(refusal(
            "flash-device-too-small",
            format!("device={} image={image_bytes}", device.bytes),
        ));
    }
    require_unmounted(&device.path)?;
    require_noninteractive_privilege()?;
    run(
        Command::new("sudo")
            .args([
                "-n",
                "dd",
                "iflag=fullblock",
                "bs=4M",
                "conv=fsync",
                "status=progress",
            ])
            .arg(format!("if={}", image.display()))
            .arg(format!("of={}", device.path.display())),
        "flash-write-failed",
    )?;
    run(
        Command::new("sudo")
            .args(["-n", "cmp", "-n", &image_bytes.to_string()])
            .arg(&image)
            .arg(&device.path),
        "flash-byte-verification-failed",
    )?;
    let record = FlashRecord {
        schema: "conduit.conduitos.armv6-rpi-flash/v1",
        base_commit: git_head(&paths.root)?,
        architecture: "armv6",
        board: board.id(),
        device: device.path.display().to_string(),
        removable: true,
        device_bytes: device.bytes,
        image_sha256: sha256_file(&image)?,
        image_bytes,
        write_completed: true,
        byte_verification_completed: true,
        physical_boot_claimed: false,
    };
    let encoded = serde_json::to_vec_pretty(&record)
        .map_err(|error| refusal("flash-record-failed", error))?;
    fs::write(
        paths
            .target
            .join(format!("{}-flash.json", board.artifact_slug())),
        &encoded,
    )
    .map_err(|error| refusal("flash-record-failed", error))?;
    if opts.json {
        println!(
            "{}",
            String::from_utf8(encoded).map_err(|error| refusal("flash-record-failed", error))?
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

struct DeviceFacts {
    path: PathBuf,
    bytes: u64,
}

fn inspect_device(requested: &Path) -> Result<DeviceFacts, ConduitosError> {
    let path =
        fs::canonicalize(requested).map_err(|error| refusal("flash-device-invalid", error))?;
    if !path.starts_with("/dev") {
        return Err(refusal(
            "flash-device-invalid",
            format!("{} is not beneath /dev", path.display()),
        ));
    }
    let metadata = fs::metadata(&path).map_err(|error| refusal("flash-device-invalid", error))?;
    if !metadata.file_type().is_block_device() {
        return Err(refusal(
            "flash-device-invalid",
            format!("{} is not a block device", path.display()),
        ));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("flash-device-invalid", "device name is not UTF-8"))?;
    let sysfs = Path::new("/sys/class/block").join(name);
    if sysfs.join("partition").exists() {
        return Err(refusal(
            "flash-device-is-partition",
            format!("{} is not a whole disk", path.display()),
        ));
    }
    let removable = read_trimmed(&sysfs.join("removable"))?;
    if removable != "1" {
        return Err(refusal(
            "flash-device-not-removable",
            format!("{} reports removable={removable}", path.display()),
        ));
    }
    let sectors = read_trimmed(&sysfs.join("size"))?
        .parse::<u64>()
        .map_err(|error| refusal("flash-device-invalid", error))?;
    Ok(DeviceFacts {
        path,
        bytes: sectors
            .checked_mul(512)
            .ok_or_else(|| refusal("flash-device-invalid", "device size overflow"))?,
    })
}

fn require_unmounted(device: &Path) -> Result<(), ConduitosError> {
    let output = Command::new("lsblk")
        .args(["-nrpo", "NAME,MOUNTPOINT"])
        .arg(device)
        .output()
        .map_err(|error| refusal("flash-device-inspection-failed", error))?;
    if !output.status.success() {
        return Err(refusal("flash-device-inspection-failed", output.status));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| refusal("flash-device-inspection-failed", error))?;
    if text
        .lines()
        .any(|line| line.split_whitespace().nth(1).is_some())
    {
        return Err(refusal(
            "flash-device-mounted",
            format!("{} or one of its partitions is mounted", device.display()),
        ));
    }
    Ok(())
}

fn require_noninteractive_privilege() -> Result<(), ConduitosError> {
    let status = Command::new("sudo")
        .args(["-n", "true"])
        .status()
        .map_err(|error| refusal("flash-privilege-unavailable", error))?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal(
            "flash-privilege-unavailable",
            "sudo credentials are not cached; run `sudo -v` interactively, then repeat the exact cargo xtask command",
        ))
    }
}

fn read_trimmed(path: &Path) -> Result<String, ConduitosError> {
    fs::read_to_string(path)
        .map(|value| value.trim().to_owned())
        .map_err(|error| refusal("flash-device-inspection-failed", error))
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
    fn mismatched_confirmation_refuses_before_device_inspection() {
        let error = execute(
            Armv6RpiBoard::BPlusV1_2,
            Path::new("/dev/sda"),
            Path::new("/dev/sdb"),
            &GlobalOpts::default(),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("flash-device-confirmation-mismatch"));
    }

    #[test]
    fn ordinary_file_is_not_a_flash_target() {
        assert!(inspect_device(Path::new("Cargo.toml")).is_err());
    }
}
