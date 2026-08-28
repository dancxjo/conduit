//! Exact generated and flashed identity for the finite Pico W appliance image.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

use super::doctor::{sha256_file, CYW43_ASSETS, CYW43_COMMIT};
use super::firmware::{identity_manifest_path, AssetEntry, PROFILE, TARGET};
use super::PicoResult;

pub const APPLIANCE_HIL_CLIENT_ARTIFACT: &str = "pico/appliance-hil-client-firmware@1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplianceFirmwareIdentity {
    pub schema: String,
    pub git_revision: String,
    pub target: String,
    pub profile: String,
    pub firmware_mode: String,
    pub firmware_build_id: String,
    pub firmware_sha256: String,
    pub appliance_image: ApplianceGeneratedImageIdentity,
    pub cyw43_commit: String,
    pub cyw43_assets: Vec<AssetEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplianceGeneratedImageIdentity {
    pub schema: String,
    pub firmware_mode: String,
    pub firmware_build_id: String,
    pub image_artifact: String,
    pub service_artifacts: Vec<String>,
    pub host_advertisement: conduit_core::HostAdvertisement,
    pub ssid: String,
    pub open_ap: bool,
    pub channel: u8,
    pub server_address: [u8; 4],
    pub local_name: String,
    pub hello_body: String,
    pub maximum_associations: u16,
    pub maximum_dhcp_leases: u16,
    pub maximum_dhcp_packet_bytes: usize,
    pub maximum_dns_packet_bytes: u32,
    pub maximum_http_request_bytes: u32,
    pub maximum_http_response_bytes: u32,
    pub maximum_signs: u16,
    pub maximum_network_sockets: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplianceHilClientFirmwareIdentity {
    pub schema: String,
    pub git_revision: String,
    pub target: String,
    pub profile: String,
    pub firmware_mode: String,
    pub firmware_build_id: String,
    pub firmware_sha256: String,
    pub client_image: ApplianceHilClientGeneratedImageIdentity,
    pub cyw43_commit: String,
    pub cyw43_assets: Vec<AssetEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplianceHilClientGeneratedImageIdentity {
    pub schema: String,
    pub firmware_mode: String,
    pub firmware_build_id: String,
    pub image_artifact: String,
    pub fixture_only: bool,
    pub usb_serial: String,
    pub ssid: String,
    pub open_ap: bool,
    pub server_address: [u8; 4],
    pub local_name: String,
    pub hello_body: String,
    pub maximum_http_request_bytes: u32,
    pub maximum_http_response_bytes: u32,
}

impl ApplianceFirmwareIdentity {
    pub fn verify(&self) -> PicoResult<()> {
        let expected_advertisement =
            conduit_rp2040_network_realization::pico_appliance_advertisement(
                "pico/appliance-hello",
                "image/boot-bound-at-runtime",
                conduit_rp2040_network_realization::PicoApplianceComposition::Hello,
                conduit_rp2040_network_realization::PicoApplianceInitialization::hello_ready(),
            )
            .map_err(|error| format!("expected Pico appliance advertisement failed: {error:?}"))?;
        let expected_artifacts = [
            conduit_rp2040_network_realization::AP_SERVICE_ARTIFACT,
            conduit_rp2040_network_realization::DHCP_SERVICE_ARTIFACT,
            conduit_rp2040_network_realization::DNS_SERVICE_ARTIFACT,
            conduit_rp2040_network_realization::HTTP_SERVICE_ARTIFACT,
        ];
        let expected_radio_assets = CYW43_ASSETS
            .iter()
            .map(|(filename, sha256)| (*filename, *sha256))
            .collect::<Vec<_>>();
        let actual_radio_assets = self
            .cyw43_assets
            .iter()
            .map(|asset| (asset.filename.as_str(), asset.sha256.as_str()))
            .collect::<Vec<_>>();
        let image = &self.appliance_image;
        if self.schema != "conduit-pico-w-signal/appliance-identity@1"
            || self.git_revision.is_empty()
            || self.target != TARGET
            || self.profile != PROFILE
            || self.firmware_sha256.len() != 64
            || self.firmware_mode != "appliance-hello"
            || image.schema != "conduit.pico-appliance/generated-image@1"
            || image.firmware_mode != self.firmware_mode
            || image.firmware_build_id != self.firmware_build_id
            || image.image_artifact != conduit_rp2040_network_realization::PICO_APPLIANCE_ARTIFACT
            || image
                .service_artifacts
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                != expected_artifacts
            || image.host_advertisement != expected_advertisement
            || image.ssid != conduit_rp2040_network_realization::APPLIANCE_SSID
            || !image.open_ap
            || image.channel != 6
            || image.server_address != conduit_rp2040_network_realization::DHCP_SERVER_ADDRESS
            || image.local_name != conduit_rp2040_network_realization::APPLIANCE_LOCAL_NAME
            || image.hello_body != conduit_rp2040_network_realization::APPLIANCE_HELLO_BODY
            || image.maximum_associations
                != conduit_rp2040_network_realization::MAXIMUM_AP_ASSOCIATIONS
            || image.maximum_dhcp_leases != conduit_rp2040_network_realization::MAXIMUM_DHCP_LEASES
            || image.maximum_dhcp_packet_bytes
                != conduit_rp2040_network_realization::MAXIMUM_DHCP_PACKET_BYTES
            || image.maximum_dns_packet_bytes
                != conduit_rp2040_network_realization::MAXIMUM_DNS_PACKET_BYTES
            || image.maximum_http_request_bytes
                != conduit_rp2040_network_realization::MAXIMUM_HTTP_REQUEST_BYTES
            || image.maximum_http_response_bytes
                != conduit_rp2040_network_realization::MAXIMUM_HTTP_RESPONSE_BYTES
            || image.maximum_signs != conduit_rp2040_network_realization::MAXIMUM_APPLIANCE_SIGNS
            || image.maximum_network_sockets
                != conduit_rp2040_network_realization::MAXIMUM_APPLIANCE_NETWORK_SOCKETS
            || self.cyw43_commit != CYW43_COMMIT
            || actual_radio_assets != expected_radio_assets
        {
            return Err("Pico appliance firmware identity is inconsistent".into());
        }
        Ok(())
    }
}

impl ApplianceHilClientFirmwareIdentity {
    pub fn verify(&self) -> PicoResult<()> {
        let expected_radio_assets = CYW43_ASSETS
            .iter()
            .map(|(filename, sha256)| (*filename, *sha256))
            .collect::<Vec<_>>();
        let actual_radio_assets = self
            .cyw43_assets
            .iter()
            .map(|asset| (asset.filename.as_str(), asset.sha256.as_str()))
            .collect::<Vec<_>>();
        let image = &self.client_image;
        if self.schema != "conduit-pico-w-signal/appliance-hil-client-identity@1"
            || self.git_revision.is_empty()
            || self.target != TARGET
            || self.profile != PROFILE
            || self.firmware_sha256.len() != 64
            || self.firmware_mode != "appliance-hil-client"
            || image.schema != "conduit.pico-appliance/hil-client-image@1"
            || image.firmware_mode != self.firmware_mode
            || image.firmware_build_id != self.firmware_build_id
            || image.image_artifact != APPLIANCE_HIL_CLIENT_ARTIFACT
            || !image.fixture_only
            || image.usb_serial != "conduit-pico-hil-client"
            || image.ssid != conduit_rp2040_network_realization::APPLIANCE_SSID
            || !image.open_ap
            || image.server_address != conduit_rp2040_network_realization::DHCP_SERVER_ADDRESS
            || image.local_name != conduit_rp2040_network_realization::APPLIANCE_LOCAL_NAME
            || image.hello_body != conduit_rp2040_network_realization::APPLIANCE_HELLO_BODY
            || image.maximum_http_request_bytes
                != conduit_rp2040_network_realization::MAXIMUM_HTTP_REQUEST_BYTES
            || image.maximum_http_response_bytes
                != conduit_rp2040_network_realization::MAXIMUM_HTTP_RESPONSE_BYTES
            || self.cyw43_commit != CYW43_COMMIT
            || actual_radio_assets != expected_radio_assets
        {
            return Err("Pico appliance HIL client firmware identity is inconsistent".into());
        }
        Ok(())
    }
}

pub fn write_appliance_identity_manifest(
    root: &Path,
    elf: &Path,
    sidecar: &Path,
) -> PicoResult<()> {
    let git_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !git_output.status.success() {
        return Err("git rev-parse HEAD failed".into());
    }
    let appliance_image: ApplianceGeneratedImageIdentity =
        serde_json::from_str(&std::fs::read_to_string(sidecar)?)?;
    let identity = ApplianceFirmwareIdentity {
        schema: "conduit-pico-w-signal/appliance-identity@1".into(),
        git_revision: String::from_utf8(git_output.stdout)?.trim().to_owned(),
        target: TARGET.into(),
        profile: PROFILE.into(),
        firmware_mode: appliance_image.firmware_mode.clone(),
        firmware_build_id: appliance_image.firmware_build_id.clone(),
        firmware_sha256: sha256_file(elf)?,
        appliance_image,
        cyw43_commit: CYW43_COMMIT.into(),
        cyw43_assets: CYW43_ASSETS
            .iter()
            .map(|(filename, expected)| AssetEntry {
                filename: (*filename).to_owned(),
                sha256: (*expected).to_owned(),
            })
            .collect(),
    };
    identity.verify()?;
    let manifest_path = identity_manifest_path(root);
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&identity)?)?;
    println!("  appliance identity manifest: {}", manifest_path.display());
    Ok(())
}

pub fn write_appliance_hil_client_identity_manifest(
    root: &Path,
    elf: &Path,
    sidecar: &Path,
) -> PicoResult<()> {
    let git_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !git_output.status.success() {
        return Err("git rev-parse HEAD failed".into());
    }
    let client_image: ApplianceHilClientGeneratedImageIdentity =
        serde_json::from_str(&std::fs::read_to_string(sidecar)?)?;
    let identity = ApplianceHilClientFirmwareIdentity {
        schema: "conduit-pico-w-signal/appliance-hil-client-identity@1".into(),
        git_revision: String::from_utf8(git_output.stdout)?.trim().to_owned(),
        target: TARGET.into(),
        profile: PROFILE.into(),
        firmware_mode: client_image.firmware_mode.clone(),
        firmware_build_id: client_image.firmware_build_id.clone(),
        firmware_sha256: sha256_file(elf)?,
        client_image,
        cyw43_commit: CYW43_COMMIT.into(),
        cyw43_assets: CYW43_ASSETS
            .iter()
            .map(|(filename, expected)| AssetEntry {
                filename: (*filename).to_owned(),
                sha256: (*expected).to_owned(),
            })
            .collect(),
    };
    identity.verify()?;
    let manifest_path = identity_manifest_path(root);
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&identity)?)?;
    println!(
        "  appliance HIL client identity manifest: {}",
        manifest_path.display()
    );
    Ok(())
}

pub fn read_appliance_identity_manifest(root: &Path) -> PicoResult<ApplianceFirmwareIdentity> {
    let manifest_path = identity_manifest_path(root);
    let identity: ApplianceFirmwareIdentity = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path).map_err(|error| {
            format!(
                "failed to read Pico appliance identity at {}: {error}; run `cargo xtask pico build --appliance-hello` first",
                manifest_path.display()
            )
        })?,
    )?;
    identity.verify()?;
    Ok(identity)
}

pub fn read_appliance_hil_client_identity_manifest(
    root: &Path,
) -> PicoResult<ApplianceHilClientFirmwareIdentity> {
    let manifest_path = identity_manifest_path(root);
    let identity: ApplianceHilClientFirmwareIdentity = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path).map_err(|error| {
            format!(
                "failed to read Pico appliance HIL client identity at {}: {error}; run `cargo xtask pico build --appliance-hil-client` first",
                manifest_path.display()
            )
        })?,
    )?;
    identity.verify()?;
    Ok(identity)
}
