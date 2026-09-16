//! Installed product entrance for reviewed artifact carriers.

use crate::cli::CarrierCommand;
use conduit_host_fabrication::{
    download_body_bound_artifact, flash_body_bound_rp2040_uf2, launch_body_bound_virtual_machine,
    write_body_bound_artifact_to_removable, QemuX86_64Launcher, CONDUITOS_X86_64_QEMU_PROFILE,
};
use serde::de::DeserializeOwned;
use std::{fs, path::Path};

const MAXIMUM_IDENTITY_DOCUMENT_BYTES: u64 = 64 * 1024;

pub(crate) fn run(command: CarrierCommand) -> Result<(), String> {
    match command {
        CarrierCommand::Download {
            descriptor,
            artifact,
            source,
            destination,
        } => {
            let descriptor = read_json(&descriptor)?;
            let artifact = read_json(&artifact)?;
            let receipt =
                download_body_bound_artifact(&descriptor, &artifact, &source, &destination)
                    .map_err(|error| format!("artifact download refused: {error:?}"))?;
            print_receipt(&receipt)
        }
        CarrierCommand::FlashUf2 {
            descriptor,
            artifact,
            source,
            volume,
            confirm_volume,
            authorize_flash,
        } => {
            let descriptor = read_json(&descriptor)?;
            let artifact = read_json(&artifact)?;
            let receipt = flash_body_bound_rp2040_uf2(
                &descriptor,
                &artifact,
                &source,
                &volume,
                &confirm_volume,
                authorize_flash,
            )
            .map_err(|error| format!("RP2040 UF2 flash refused: {error:?}"))?;
            print_receipt(&receipt)
        }
        CarrierCommand::LaunchVm {
            descriptor,
            artifact,
            source,
            authorize_launch,
        } => {
            let descriptor = read_json(&descriptor)?;
            let artifact = read_json(&artifact)?;
            let mut launcher = QemuX86_64Launcher::new("qemu-system-x86_64");
            let receipt = launch_body_bound_virtual_machine(
                &descriptor,
                &artifact,
                &source,
                CONDUITOS_X86_64_QEMU_PROFILE,
                authorize_launch,
                &mut launcher,
            )
            .map_err(|error| format!("virtual-machine launch refused: {error:?}"))?;
            print_receipt(&receipt)
        }
        CarrierCommand::WriteRemovable {
            descriptor,
            artifact,
            source,
            destination,
            confirm_destination,
            authorize_write,
        } => {
            let descriptor = read_json(&descriptor)?;
            let artifact = read_json(&artifact)?;
            let receipt = write_body_bound_artifact_to_removable(
                &descriptor,
                &artifact,
                &source,
                &destination,
                &confirm_destination,
                authorize_write,
            )
            .map_err(|error| format!("removable-device write refused: {error:?}"))?;
            print_receipt(&receipt)
        }
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("inspect {}: {error}", path.display()))?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAXIMUM_IDENTITY_DOCUMENT_BYTES
    {
        return Err(format!(
            "{} violates the identity document bound",
            path.display()
        ));
    }
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("decode {}: {error}", path.display()))
}

fn print_receipt(receipt: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(receipt)
            .map_err(|error| format!("encode carrier receipt: {error}"))?
    );
    Ok(())
}
