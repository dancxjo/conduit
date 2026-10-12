//! Volatile provider state; stable installed Offers do not establish residency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModelResidencyState {
    Unknown,
    NotResident,
    /// Loaded is observable, but no continuous residency epoch is supplied.
    ResidentWithoutEpoch,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalModelRuntimeState {
    pub residency: ModelResidencyState,
    /// Independently queried provider version; not a Boot or residency epoch.
    pub runtime_version: Option<String>,
    pub resident_bytes: Option<u64>,
    pub vram_bytes: Option<u64>,
    pub context_length: Option<u64>,
    /// Provider wall-clock spelling; never a Host monotonic expiry.
    pub provider_expiry: Option<String>,
}
impl LocalModelRuntimeState {
    pub fn unknown() -> Self {
        Self {
            residency: ModelResidencyState::Unknown,
            runtime_version: None,
            resident_bytes: None,
            vram_bytes: None,
            context_length: None,
            provider_expiry: None,
        }
    }
}
