use conduit_host_fabrication::BaseSelection;
use serde::Serialize;

use crate::process::ExecutedCommand;

#[derive(Debug, Serialize)]
pub struct CheckReceipt {
    pub schema: &'static str,
    pub outcome: &'static str,
    pub proof_class: &'static str,
    pub source_sha: String,
    pub input_state: &'static str,
    pub dirty_status_sha256: Option<String>,
    pub tracked_input_count: usize,
    pub tracked_inputs_sha256: String,
    pub cargo_build_jobs: Option<String>,
    pub lock_sha256: String,
    pub architecture_descriptor_sha256: String,
    pub cargo_config_sha256: String,
    pub architecture_package: String,
    pub architecture_revision: u32,
    pub builder_adapter: String,
    pub declared_toolchain: String,
    pub observed_toolchain: String,
    pub observed_toolchain_sha256: String,
    pub target: String,
    pub chip: String,
    pub board_descriptor: String,
    pub minimal_bases: Vec<BaseSelection>,
    pub full_bases: Vec<BaseSelection>,
    pub minimal_features: Vec<String>,
    pub full_features: Vec<String>,
    pub minimal_runtime_packages: Vec<String>,
    pub full_runtime_packages: Vec<String>,
    pub artifact_sha256: Option<String>,
    pub executed_commands: Vec<ExecutedCommand>,
    pub check_identity: String,
    pub excluded_truth: [&'static str; 7],
}

pub const EXCLUDED_TRUTH: [&str; 7] = [
    "physical-boot",
    "host-id",
    "boot-id",
    "host-offer",
    "line-readiness",
    "peripheral-readiness",
    "flash-success",
];
