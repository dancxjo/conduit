use std::{fs, path::Path};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    armv6_rpi_b_plus_image,
    armv6_rpi_board::Armv6RpiBoard,
    profile::Paths,
    removable_media,
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
    let device = removable_media::inspect_confirmed(requested, confirmed)?;
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
    removable_media::write_and_verify(&image, image_bytes, &device)?;
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
}
