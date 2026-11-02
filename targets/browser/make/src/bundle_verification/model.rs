use conduit_host_make::HostReleaseFile;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Implementation {
    pub id: String,
    pub revision: u32,
    pub artifact: String,
}
#[derive(Deserialize)]
pub(super) struct Module {
    pub path: String,
    pub dependencies: Vec<String>,
}
#[derive(Deserialize)]
pub(super) struct Distribution {
    pub schema: String,
    pub distribution_id: String,
    pub runtime_abi: String,
    pub targets: Vec<String>,
    pub toolchain_identity: String,
    pub source_commit: String,
    pub maximum_bundle_bytes: u64,
    pub implementations: Vec<Implementation>,
    pub modules: Vec<Module>,
}

// Field order is the existing JS JSON.stringify canonical identity contract.
// Keep differential fixtures against browser-bundle.mjs when changing these.
#[derive(Serialize)]
pub(super) struct BuildBasis<'a> {
    pub schema: &'static str,
    pub target_id: &'static str,
    pub profile_id: &'a str,
    pub source_configuration_id: &'a str,
    pub distribution_id: &'a str,
    pub distribution_digest: &'a str,
    pub runtime_abi: &'a str,
    pub implementations: &'a [Implementation],
}
#[derive(Serialize)]
pub(super) struct DistributionBinding<'a> {
    pub distribution_id: &'a str,
    pub distribution_digest: &'a str,
    pub runtime_abi: &'a str,
    pub toolchain_identity: &'a str,
    pub source_commit: &'a str,
}
#[derive(Serialize)]
pub(super) struct BootModule<'a> {
    pub role: &'static str,
    pub path: &'static str,
    pub sha256: &'a str,
}
#[derive(Serialize)]
pub(super) struct ImagePayload<'a> {
    pub schema: &'static str,
    pub build_id: &'a str,
    pub target_id: &'static str,
    pub profile_id: &'a str,
    pub source_configuration_id: &'a str,
    pub reviewed_distribution: DistributionBinding<'a>,
    pub implementations: &'a [Implementation],
    pub boot_module: BootModule<'a>,
    pub files: &'a [HostReleaseFile],
}
