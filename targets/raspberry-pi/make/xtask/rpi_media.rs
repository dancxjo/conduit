//! Shared Raspberry Pi SD geometry and fixed boot-file timestamps.
use super::ConduitosError;
use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::Path,
    process::Command,
};
pub(super) const IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const SECTOR_BYTES: u64 = 512;
pub(super) const PARTITION_START_SECTOR: u32 = 2048;
pub(super) const PARTITION_SECTORS: u32 =
    (IMAGE_BYTES / SECTOR_BYTES) as u32 - PARTITION_START_SECTOR;
pub(super) const FAT_OFFSET_BYTES: u64 = PARTITION_START_SECTOR as u64 * SECTOR_BYTES;
const SOURCE_DATE_EPOCH: &str = "1786233600";

pub(super) fn write_partitioned_image(path: &Path) -> Result<(), ConduitosError> {
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

pub(super) fn verify_partition_layout(image: &Path) -> Result<(), ConduitosError> {
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
    Ok(())
}

pub(super) fn set_fixed_timestamps_for(stage: &Path, files: &[&str]) -> Result<(), ConduitosError> {
    let mut command = Command::new("touch");
    command.args(["--date", &format!("@{SOURCE_DATE_EPOCH}")]);
    for name in files {
        command.arg(stage.join(name));
    }
    run(&mut command, "image-staging-failed")
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}
fn run(command: &mut Command, reason: &'static str) -> Result<(), ConduitosError> {
    let status = command.status().map_err(|error| refusal(reason, error))?;
    if status.success() {
        Ok(())
    } else {
        Err(refusal(reason, status))
    }
}

pub(super) fn format_fat(image: &Path) -> Result<(), ConduitosError> {
    run(
        Command::new("mkfs.vfat")
            .arg("--invariant")
            .arg(format!("--offset={PARTITION_START_SECTOR}"))
            .args([
                "-F",
                "32",
                "-h",
                &PARTITION_START_SECTOR.to_string(),
                "-i",
                "434e4454",
                "-n",
                "CONDUITOS",
            ])
            .arg(image),
        "fat-filesystem-creation-failed",
    )
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new(name: &str) -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("conduit-rpi-{name}-{}-{stamp}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn partition_record_rejects_corruption_and_truncation() {
        let scratch = Scratch::new("partition");
        let image = scratch.0.join("sd.img");
        write_partitioned_image(&image).unwrap();
        verify_partition_layout(&image).unwrap();
        let mut bytes = fs::read(&image).unwrap();
        assert_eq!(bytes.len(), 64 * 1024 * 1024);
        assert_eq!(&bytes[510..512], &[0x55, 0xaa]);
        let start = u32::from_le_bytes(bytes[454..458].try_into().unwrap()) as usize * 512;
        let length = u32::from_le_bytes(bytes[458..462].try_into().unwrap()) as usize * 512;
        assert_eq!(start, 1024 * 1024);
        assert_eq!(start + length, bytes.len());
        bytes[450] = 0;
        fs::write(&image, &bytes).unwrap();
        assert!(verify_partition_layout(&image).is_err());
        File::options()
            .write(true)
            .open(&image)
            .unwrap()
            .set_len(16)
            .unwrap();
        assert!(verify_partition_layout(&image).is_err());
    }
    #[test]
    fn fixed_timestamps_use_the_selected_kernel_filename() {
        let scratch = Scratch::new("timestamps");
        for name in ["kernel8.img", "config.txt"] {
            fs::write(scratch.0.join(name), b"fixture").unwrap();
        }
        set_fixed_timestamps_for(&scratch.0, &["kernel8.img", "config.txt"]).unwrap();
        assert!(!scratch.0.join("kernel.img").exists());
        for name in ["kernel8.img", "config.txt"] {
            let epoch = fs::metadata(scratch.0.join(name))
                .unwrap()
                .modified()
                .unwrap()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            assert_eq!(epoch, 1786233600);
        }
    }
}
