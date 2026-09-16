//! Installed product entrance for reviewed artifact carriers.

use crate::cli::CarrierCommand;
use conduit_host_fabrication::{
    download_body_bound_artifact, flash_body_bound_rp2040_uf2,
    install_start_body_bound_native_package, launch_body_bound_virtual_machine,
    serve_body_bound_network_boot, write_body_bound_artifact_to_removable,
    DeploymentCarrierDescriptor, DeploymentCarrierKind, HttpBootServer, NetworkBootServeBounds,
    QemuX86_64Launcher, CONDUITOS_X86_64_QEMU_PROFILE,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;
use std::{fs, path::Path};

const MAXIMUM_IDENTITY_DOCUMENT_BYTES: u64 = 64 * 1024;

pub(crate) fn run(command: CarrierCommand) -> Result<(), String> {
    match command {
        CarrierCommand::Availability { descriptors } => report_availability(&descriptors),
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
        CarrierCommand::InstallNative {
            descriptor,
            artifact,
            source,
            state_dir,
            authorize_install,
        } => {
            let descriptor = read_json(&descriptor)?;
            let artifact = read_json(&artifact)?;
            let mut installer = crate::native_package_install::LocalNativeInstaller::new(state_dir);
            let receipt = install_start_body_bound_native_package(
                &descriptor,
                &artifact,
                &source,
                authorize_install,
                &mut installer,
            )
            .map_err(|error| format!("native install/start refused: {error:?}"))?;
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
        CarrierCommand::ServeHttpBoot {
            descriptor,
            artifact,
            source,
            bind,
            maximum_requests,
            timeout_seconds,
            authorize_serve,
        } => {
            let descriptor = read_json(&descriptor)?;
            let artifact = read_json(&artifact)?;
            let address = bind
                .parse()
                .map_err(|error| format!("HTTP Boot bind address is invalid: {error}"))?;
            let mut server = HttpBootServer::new(address);
            let receipt = serve_body_bound_network_boot(
                &descriptor,
                &artifact,
                &source,
                authorize_serve,
                NetworkBootServeBounds {
                    maximum_requests,
                    timeout: Duration::from_secs(timeout_seconds),
                },
                &mut server,
            )
            .map_err(|error| format!("HTTP Boot carrier refused: {error:?}"))?;
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

#[derive(Debug, Serialize)]
struct CarrierAvailability {
    schema: &'static str,
    target_id: String,
    artifact_download_available: bool,
    local_realization_available: bool,
    carriers: Vec<DeploymentCarrierDescriptor>,
}

fn report_availability(paths: &[std::path::PathBuf]) -> Result<(), String> {
    let carriers = paths
        .iter()
        .map(|path| read_json::<DeploymentCarrierDescriptor>(path))
        .collect::<Result<Vec<_>, _>>()?;
    print_receipt(&collect_availability(carriers)?)
}

fn collect_availability(
    mut carriers: Vec<DeploymentCarrierDescriptor>,
) -> Result<CarrierAvailability, String> {
    let target = carriers
        .first()
        .ok_or("at least one reviewed carrier descriptor is required")?
        .target_id
        .clone();
    for descriptor in &carriers {
        descriptor
            .validate()
            .map_err(|error| format!("carrier descriptor refused: {error:?}"))?;
        if descriptor.target_id != target {
            return Err("carrier descriptors belong to different targets".into());
        }
    }
    carriers.sort_by(|left, right| left.carrier_id.cmp(&right.carrier_id));
    carriers.dedup_by(|left, right| left.carrier_id == right.carrier_id);
    let artifact_download_available = carriers
        .iter()
        .any(|carrier| carrier.kind == DeploymentCarrierKind::ArtifactDownload);
    let local_realization_available = carriers.iter().any(|carrier| carrier.kind.consequential());
    Ok(CarrierAvailability {
        schema: "conduit.carrier/availability@1",
        target_id: target,
        artifact_download_available,
        local_realization_available,
        carriers,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(
        target: &str,
        carrier_id: &str,
        kind: DeploymentCarrierKind,
    ) -> DeploymentCarrierDescriptor {
        DeploymentCarrierDescriptor {
            schema: "conduit.carrier/descriptor@1".into(),
            carrier_id: carrier_id.into(),
            target_id: target.into(),
            kind,
            implementation_id: format!("implementation/{carrier_id}"),
            maximum_artifact_bytes: 1024,
            requires_explicit_authority: kind.consequential(),
            verifies_written_bytes: false,
        }
    }

    #[test]
    fn availability_distinguishes_download_only_from_local_realization() {
        let download = descriptor(
            "target/one",
            "carrier/download",
            DeploymentCarrierKind::ArtifactDownload,
        );
        let download_only = collect_availability(vec![download.clone()]).unwrap();
        assert!(download_only.artifact_download_available);
        assert!(!download_only.local_realization_available);

        let realized = collect_availability(vec![
            descriptor(
                "target/one",
                "carrier/native",
                DeploymentCarrierKind::NativeInstallStart,
            ),
            download,
        ])
        .unwrap();
        assert!(realized.artifact_download_available);
        assert!(realized.local_realization_available);
        assert_eq!(realized.carriers[0].carrier_id, "carrier/download");
    }

    #[test]
    fn availability_refuses_mixed_target_inventories() {
        let error = collect_availability(vec![
            descriptor(
                "target/one",
                "carrier/download",
                DeploymentCarrierKind::ArtifactDownload,
            ),
            descriptor(
                "target/two",
                "carrier/native",
                DeploymentCarrierKind::NativeInstallStart,
            ),
        ])
        .unwrap_err();
        assert_eq!(error, "carrier descriptors belong to different targets");
    }
}
