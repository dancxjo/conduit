//! Shared whole-device safety boundary for physical ConduitOS media writes.

use std::{
    fs,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    process::Command,
};

use super::ConduitosError;

#[derive(Debug)]
pub(super) struct DeviceFacts {
    pub(super) path: PathBuf,
    pub(super) bytes: u64,
}

pub(super) fn inspect_confirmed(
    requested: &Path,
    confirmed: &Path,
) -> Result<DeviceFacts, ConduitosError> {
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

pub(super) fn write_and_verify(
    image: &Path,
    image_bytes: u64,
    device: &DeviceFacts,
) -> Result<(), ConduitosError> {
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
            .arg(image)
            .arg(&device.path),
        "flash-byte-verification-failed",
    )
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
        let error = inspect_confirmed(Path::new("/dev/sda"), Path::new("/dev/sdb")).unwrap_err();
        assert!(error
            .to_string()
            .contains("flash-device-confirmation-mismatch"));
    }

    #[test]
    fn ordinary_file_is_not_a_flash_target() {
        assert!(inspect_confirmed(Path::new("Cargo.toml"), Path::new("Cargo.toml")).is_err());
    }

    #[test]
    fn undersized_device_refuses_before_mount_or_privilege_checks() {
        let device = DeviceFacts {
            path: PathBuf::from("/dev/absent-test-device"),
            bytes: 0,
        };
        let error = write_and_verify(Path::new("Cargo.toml"), 1, &device).unwrap_err();
        assert!(error.to_string().contains("flash-device-too-small"));
    }
}
