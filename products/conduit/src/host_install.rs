//! Target-oriented host installation planning and realization.

use conduit_host_make::{
    download_body_bound_artifact, flash_body_bound_rp2040_uf2,
    install_start_body_bound_native_package, launch_body_bound_virtual_machine,
    serve_body_bound_network_boot, write_body_bound_artifact_to_removable,
    BodyBoundArtifactIdentity, DeploymentCarrierDescriptor, DeploymentCarrierKind, HttpBootServer,
    NetworkBootServeBounds, QemuX86_64Launcher, CONDUITOS_X86_64_QEMU_PROFILE,
};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

pub(crate) struct InstallRequest<'a> {
    pub(crate) target: &'a str,
    pub(crate) catalog: &'a Path,
    pub(crate) catalog_id: &'a str,
    pub(crate) mirror: &'a Path,
    pub(crate) cache: &'a Path,
    pub(crate) carrier_descriptors: &'a [PathBuf],
    pub(crate) carrier: Option<&'a str>,
    pub(crate) minimum_generation: u64,
    pub(crate) realization_request: Option<&'a Path>,
    pub(crate) dry_run: bool,
}

#[derive(Debug, Serialize)]
struct InstallPlan {
    schema: &'static str,
    target_id: String,
    acquisition: crate::release_obtain::ObtainReceipt,
    selected_carrier: DeploymentCarrierDescriptor,
    stages: [&'static str; 5],
    required_authority: bool,
    carrier_effects_performed: bool,
    durable_host_observed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RealizationRequest {
    schema: String,
    artifact: BodyBoundArtifactIdentity,
    source: PathBuf,
    explicit_authority: bool,
    realization: RealizationConfiguration,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum RealizationConfiguration {
    ArtifactDownload {
        destination: PathBuf,
    },
    NativeInstall {
        state_dir: PathBuf,
    },
    MicrocontrollerFlash {
        volume: PathBuf,
        confirm_volume: String,
    },
    VirtualMachineLaunch,
    NetworkBootServe {
        bind: SocketAddr,
        maximum_requests: u16,
        timeout_seconds: u64,
    },
    RemovableWholeDeviceWrite {
        destination: PathBuf,
        confirm_destination: String,
    },
}

#[derive(Debug, Serialize)]
struct ObservedHostIdentity {
    host_id: String,
    release_bundle_sha256: String,
}

#[derive(Debug, Serialize)]
struct InstallReceipt {
    schema: &'static str,
    target_id: String,
    acquisition: crate::release_obtain::ObtainReceipt,
    selected_carrier: DeploymentCarrierDescriptor,
    authorization_observed: bool,
    realization: serde_json::Value,
    installation_outcome: &'static str,
    activation_outcome: &'static str,
    observed_host: Option<ObservedHostIdentity>,
    durable_host_observed: bool,
    boot_observed: bool,
}

pub(crate) fn run(request: InstallRequest<'_>) -> Result<(), String> {
    let obtained = crate::release_obtain::obtain(
        request.target,
        request.catalog,
        request.catalog_id,
        request.mirror,
        request.cache,
        request.minimum_generation,
    )
    .map_err(|error| format!("acquisition failed: {error}"))?;
    let selected = select_carrier(request.target, request.carrier_descriptors, request.carrier)
        .map_err(|error| format!("carrier availability failed: {error}"))?;
    if request.dry_run {
        return print_json(&InstallPlan {
            schema: "conduit.host/install-plan@1",
            target_id: request.target.into(),
            acquisition: obtained,
            required_authority: selected.requires_explicit_authority,
            selected_carrier: selected,
            stages: [
                "release-acquisition",
                "carrier-selection",
                "carrier-authorization",
                "carrier-realization",
                "host-observation",
            ],
            carrier_effects_performed: false,
            durable_host_observed: false,
        });
    }
    let path = request
        .realization_request
        .ok_or("installation request is required when effects are enabled")?;
    let realization: RealizationRequest = crate::deployment_carrier::read_json(path)
        .map_err(|error| format!("installation request unavailable: {error}"))?;
    validate_request(request.target, &selected, &realization)?;
    let (carrier_receipt, observed_host) = realize(&selected, &realization)?;
    let native = selected.kind == DeploymentCarrierKind::NativeInstallStart;
    let boot_observed = receipt_boot_observed(&carrier_receipt);
    let authorization_observed = receipt_bool(&carrier_receipt, "explicit_authority_observed");
    print_json(&InstallReceipt {
        schema: "conduit.host/install-receipt@1",
        target_id: request.target.into(),
        acquisition: obtained,
        selected_carrier: selected,
        authorization_observed,
        realization: carrier_receipt,
        installation_outcome: if native {
            "installed"
        } else {
            "not-applicable"
        },
        activation_outcome: if native {
            "activated"
        } else {
            "not-applicable"
        },
        durable_host_observed: observed_host.is_some(),
        observed_host,
        boot_observed,
    })
}

fn validate_request(
    target: &str,
    carrier: &DeploymentCarrierDescriptor,
    request: &RealizationRequest,
) -> Result<(), String> {
    if request.schema != "conduit.host/install-request@1" {
        return Err("installation request schema is unsupported".into());
    }
    if request.artifact.target_id != target {
        return Err("installation request artifact belongs to another target".into());
    }
    let matches = matches!(
        (carrier.kind, &request.realization),
        (
            DeploymentCarrierKind::ArtifactDownload,
            RealizationConfiguration::ArtifactDownload { .. }
        ) | (
            DeploymentCarrierKind::NativeInstallStart,
            RealizationConfiguration::NativeInstall { .. }
        ) | (
            DeploymentCarrierKind::MicrocontrollerFlash,
            RealizationConfiguration::MicrocontrollerFlash { .. }
        ) | (
            DeploymentCarrierKind::VirtualMachineLaunch,
            RealizationConfiguration::VirtualMachineLaunch
        ) | (
            DeploymentCarrierKind::NetworkBootServe,
            RealizationConfiguration::NetworkBootServe { .. }
        ) | (
            DeploymentCarrierKind::RemovableWholeDeviceWrite,
            RealizationConfiguration::RemovableWholeDeviceWrite { .. }
        )
    );
    if !matches {
        return Err("installation request does not match the selected carrier".into());
    }
    if carrier.requires_explicit_authority && !request.explicit_authority {
        return Err("carrier authorization failed: explicit authority is required".into());
    }
    Ok(())
}

fn realize(
    descriptor: &DeploymentCarrierDescriptor,
    request: &RealizationRequest,
) -> Result<(serde_json::Value, Option<ObservedHostIdentity>), String> {
    let artifact = &request.artifact;
    let source = &request.source;
    let authority = request.explicit_authority;
    match &request.realization {
        RealizationConfiguration::ArtifactDownload { destination } => {
            let receipt = download_body_bound_artifact(descriptor, artifact, source, destination)
                .map_err(|error| format!("carrier realization failed: {error:?}"))?;
            Ok((json(&receipt)?, None))
        }
        RealizationConfiguration::NativeInstall { state_dir } => {
            let mut installer =
                crate::native_package_install::LocalNativeInstaller::new(state_dir.clone());
            realize_native(descriptor, request, &mut installer)
        }
        RealizationConfiguration::MicrocontrollerFlash {
            volume,
            confirm_volume,
        } => {
            let receipt = flash_body_bound_rp2040_uf2(
                descriptor,
                artifact,
                source,
                volume,
                confirm_volume,
                authority,
            )
            .map_err(|error| format!("carrier realization failed: {error:?}"))?;
            Ok((json(&receipt)?, None))
        }
        RealizationConfiguration::VirtualMachineLaunch => {
            let mut launcher = QemuX86_64Launcher::new("qemu-system-x86_64");
            let receipt = launch_body_bound_virtual_machine(
                descriptor,
                artifact,
                source,
                CONDUITOS_X86_64_QEMU_PROFILE,
                authority,
                &mut launcher,
            )
            .map_err(|error| format!("carrier realization failed: {error:?}"))?;
            Ok((json(&receipt)?, None))
        }
        RealizationConfiguration::NetworkBootServe {
            bind,
            maximum_requests,
            timeout_seconds,
        } => {
            let mut server = HttpBootServer::new(*bind);
            let receipt = serve_body_bound_network_boot(
                descriptor,
                artifact,
                source,
                authority,
                NetworkBootServeBounds {
                    maximum_requests: *maximum_requests,
                    timeout: Duration::from_secs(*timeout_seconds),
                },
                &mut server,
            )
            .map_err(|error| format!("carrier realization failed: {error:?}"))?;
            Ok((json(&receipt)?, None))
        }
        RealizationConfiguration::RemovableWholeDeviceWrite {
            destination,
            confirm_destination,
        } => {
            let receipt = write_body_bound_artifact_to_removable(
                descriptor,
                artifact,
                source,
                destination,
                confirm_destination,
                authority,
            )
            .map_err(|error| format!("carrier realization failed: {error:?}"))?;
            Ok((json(&receipt)?, None))
        }
    }
}

fn realize_native(
    descriptor: &DeploymentCarrierDescriptor,
    request: &RealizationRequest,
    installer: &mut crate::native_package_install::LocalNativeInstaller,
) -> Result<(serde_json::Value, Option<ObservedHostIdentity>), String> {
    let receipt = install_start_body_bound_native_package(
        descriptor,
        &request.artifact,
        &request.source,
        request.explicit_authority,
        installer,
    )
    .map_err(|error| format!("installation or activation failed: {error:?}"))?;
    let identity = installer
        .installed_host()
        .ok_or("host observation failed: activated native Host identity is absent")?;
    Ok((
        json(&receipt)?,
        Some(ObservedHostIdentity {
            host_id: identity.host_id.clone(),
            release_bundle_sha256: identity.release_bundle_sha256.clone(),
        }),
    ))
}

fn receipt_boot_observed(receipt: &serde_json::Value) -> bool {
    receipt_bool(receipt, "boot_observed")
}

fn receipt_bool(receipt: &serde_json::Value, field: &str) -> bool {
    receipt
        .get(field)
        .or_else(|| {
            receipt
                .get("realization")
                .and_then(|value| value.get(field))
        })
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn json(value: &impl Serialize) -> Result<serde_json::Value, String> {
    serde_json::to_value(value).map_err(|error| format!("encode carrier receipt: {error}"))
}

fn print_json(value: &impl Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(value)
            .map_err(|error| format!("encode host install receipt: {error}"))?
    );
    Ok(())
}

fn select_carrier(
    target: &str,
    paths: &[PathBuf],
    requested: Option<&str>,
) -> Result<DeploymentCarrierDescriptor, String> {
    let mut compatible = paths
        .iter()
        .map(|path| crate::deployment_carrier::read_json::<DeploymentCarrierDescriptor>(path))
        .collect::<Result<Vec<_>, _>>()?;
    for descriptor in &compatible {
        descriptor
            .validate()
            .map_err(|error| format!("carrier descriptor refused: {error:?}"))?;
        if descriptor.target_id != target {
            return Err(format!(
                "carrier {} belongs to target {}, not {target}",
                descriptor.carrier_id, descriptor.target_id
            ));
        }
    }
    compatible.sort_by(|left, right| left.carrier_id.cmp(&right.carrier_id));
    compatible.dedup_by(|left, right| left.carrier_id == right.carrier_id);
    if let Some(requested) = requested {
        return compatible
            .into_iter()
            .find(|descriptor| descriptor.carrier_id == requested)
            .ok_or_else(|| format!("reviewed carrier {requested} is unavailable for {target}"));
    }
    match compatible.len() {
        0 => Err(format!("no reviewed carrier is available for {target}")),
        1 => Ok(compatible.remove(0)),
        _ => Err(format!(
            "multiple reviewed carriers are available for {target}; select one exact --carrier"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::{DEPLOYMENT_CARRIER_SCHEMA, NATIVE_INSTALL_START_IMPLEMENTATION};
    use std::fs;

    fn descriptor(id: &str, target: &str) -> DeploymentCarrierDescriptor {
        DeploymentCarrierDescriptor {
            schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
            carrier_id: id.into(),
            target_id: target.into(),
            kind: DeploymentCarrierKind::NativeInstallStart,
            implementation_id: format!("test/{id}@1"),
            maximum_artifact_bytes: 1024,
            requires_explicit_authority: true,
            verifies_written_bytes: true,
        }
    }

    #[test]
    fn carrier_selection_refuses_ambiguity_and_target_mismatch() {
        let root =
            std::env::temp_dir().join(format!("conduit-install-plan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = [
            descriptor("native-a", "hosted/linux"),
            descriptor("native-b", "hosted/linux"),
        ]
        .into_iter()
        .map(|descriptor| {
            let path = root.join(format!("{}.json", descriptor.carrier_id));
            fs::write(&path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
            path
        })
        .collect::<Vec<_>>();
        assert!(select_carrier("hosted/linux", &paths, None)
            .unwrap_err()
            .contains("multiple"));
        assert_eq!(
            select_carrier("hosted/linux", &paths, Some("native-b"))
                .unwrap()
                .carrier_id,
            "native-b"
        );
        assert!(select_carrier("another/target", &paths[..1], None)
            .unwrap_err()
            .contains("belongs"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn flash_receipt_does_not_infer_boot_observation() {
        let value = serde_json::json!({
            "schema": "conduit.carrier/realization@1",
            "boot_observed": false
        });
        assert!(!receipt_boot_observed(&value));
    }

    #[test]
    fn native_realization_verifies_bundle_activates_service_and_observes_host() {
        let (package, artifact) = crate::native_package_install::tests::fixture();
        let root = std::env::temp_dir().join(format!(
            "conduit-host-install-native-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("body-bound.zip");
        let state_dir = root.join("state");
        fs::write(&source, package).unwrap();
        let descriptor = DeploymentCarrierDescriptor {
            schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
            carrier_id: "conduit-carrier/native-install-start@1".into(),
            target_id: artifact.target_id.clone(),
            kind: DeploymentCarrierKind::NativeInstallStart,
            implementation_id: NATIVE_INSTALL_START_IMPLEMENTATION.into(),
            maximum_artifact_bytes: 64 * 1024 * 1024,
            requires_explicit_authority: true,
            verifies_written_bytes: true,
        };
        let request = RealizationRequest {
            schema: "conduit.host/install-request@1".into(),
            artifact,
            source,
            explicit_authority: true,
            realization: RealizationConfiguration::NativeInstall {
                state_dir: state_dir.clone(),
            },
        };
        let mut installer =
            crate::native_package_install::LocalNativeInstaller::new_without_service_manager(
                state_dir.clone(),
            );
        let (receipt, observed) = realize_native(&descriptor, &request, &mut installer).unwrap();
        assert_eq!(receipt["terminal"], "completed");
        assert!(receipt["byte_verification_completed"].as_bool().unwrap());
        assert!(observed.unwrap().host_id.starts_with("host/installed/"));
        assert!(state_dir.join("installation.json").is_file());
        assert!(state_dir.join("conduit-host.service").is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
