//! Artifact identities and explicit proof boundaries for Raspberry Pi image records.
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct ImageRecord {
    pub(super) schema: &'static str,
    pub(super) proof_class: &'static str,
    pub(super) base_commit: String,
    pub(super) architecture: &'static str,
    pub(super) machine: &'static str,
    pub(super) kernel_binary: &'static str,
    pub(super) fixed_core_clock_hz: Option<u32>,
    pub(super) display_proof_claimed: bool,
    pub(super) button_proof_claimed: bool,
    pub(super) board: &'static str,
    pub(super) boot_mechanism: &'static str,
    pub(super) firmware_repository: &'static str,
    pub(super) firmware_commit: &'static str,
    pub(super) firmware_assets: Vec<FirmwareAssetRecord>,
    pub(super) image_path: String,
    pub(super) image_sha256: String,
    pub(super) image_bytes: u64,
    pub(super) partition_scheme: &'static str,
    pub(super) partition_start_sector: u32,
    pub(super) partition_sectors: u32,
    pub(super) filesystem: &'static str,
    pub(super) volume_label: &'static str,
    pub(super) files: [&'static str; 6],
    pub(super) kernel_image_sha256: String,
    pub(super) boot_claimed: bool,
    pub(super) physical_proof_claimed: bool,
    pub(super) target_id: String,
    pub(super) make_package_id: &'static str,
    pub(super) make_package_revision: u32,
    pub(super) output: &'static str,
    pub(super) builder_adapter: &'static str,
    pub(super) deployment_adapter: &'static str,
    pub(super) source_identity: String,
    pub(super) image_id: String,
    pub(super) artifact: ImageArtifactRecord,
    pub(super) boot_files: Vec<BootFileRecord>,
}

#[derive(Serialize)]
pub(super) struct ImageArtifactRecord {
    pub(super) path: String,
    pub(super) format: &'static str,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Serialize)]
pub(super) struct BootFileRecord {
    pub(super) path: &'static str,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Serialize)]
pub(super) struct FirmwareAssetRecord {
    pub(super) name: &'static str,
    pub(super) sha256: &'static str,
    pub(super) bytes: u64,
}
