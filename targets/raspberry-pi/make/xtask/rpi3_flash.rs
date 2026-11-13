//! Use the shared removable-device boundary for the exact Pi 3B+ image.
use super::{
    profile::Paths, removable_media, report::sha256_file, rpi3_image, ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use clap::Args;
use std::{fs, path::PathBuf};

#[derive(Args, Debug, Clone)]
pub(super) struct FlashArgs {
    /// Whole removable block device to erase and write.
    #[arg(long)]
    device: PathBuf,
    /// Repeat the device path to acknowledge erasure.
    #[arg(long)]
    confirm_device: PathBuf,
}
pub(super) fn execute(args: &FlashArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let device = removable_media::inspect_confirmed(&args.device, &args.confirm_device)?;
    if opts.dry_run {
        println!(
            "build Pi 3B+ image; erase, write and verify {}",
            device.path.display()
        );
        return Ok(());
    }
    rpi3_image::execute(opts)?;
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let image = paths.target.join("conduitos-rpi-3b-plus.img");
    let bytes = fs::metadata(&image).map_err(io_error)?.len();
    removable_media::write_and_verify(&image, bytes, &device)?;
    let receipt = serde_json::json!({
        "schema":"conduit.conduitos.rpi3-flash/v1", "architecture":"aarch64",
        "board":"raspberry-pi-3-model-b-plus-rev-1.3", "device":device.path,
        "device_bytes":device.bytes,"image_bytes":bytes,"image_sha256":sha256_file(&image)?,
        "write_completed":true,"byte_verification_completed":true,"physical_boot_claimed":false
    });
    fs::write(
        paths.target.join("rpi-3b-plus-flash.json"),
        serde_json::to_vec_pretty(&receipt).map_err(io_error)?,
    )
    .map_err(io_error)?;
    println!("Wrote and byte-verified {}", image.display());
    Ok(())
}
fn io_error(error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal("rpi3-flash-artifact-error", error.to_string())
}
