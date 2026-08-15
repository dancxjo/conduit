use serde::{Deserialize, Serialize};

pub const HOST_PROFILE_SCHEMA: &str = "conduit.host/profile@1";
pub const MAX_PROFILE_ITEMS: usize = 256;
pub const MAX_PROFILE_ID_BYTES: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostProfile {
    pub schema: String,
    pub name: String,
    pub target: TargetSelection,
    pub host_core: String,
    #[serde(default)]
    pub fragments: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<CapabilitySelection>,
    #[serde(default)]
    pub host_operations: Vec<String>,
    #[serde(default)]
    pub resources: Vec<ResourceBudget>,
    #[serde(default)]
    pub bases: Vec<BaseSelection>,
    #[serde(default)]
    pub drivers: Vec<DriverSelection>,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub presenters: Vec<PresenterSelection>,
    #[serde(default)]
    pub facilities: Vec<String>,
    #[serde(default)]
    pub exclusions: Vec<String>,
    pub policy: HostPolicy,
    pub bounds: HostBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSelection {
    pub family: String,
    pub architecture: String,
    pub machine: String,
    pub build_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabrication_descriptor: Option<String>,
}

impl TargetSelection {
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.family, self.architecture, self.machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySelection {
    pub kind: String,
    pub contract_revision: String,
    pub implementation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudget {
    pub id: String,
    pub class: String,
    pub slots: u32,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseSelection {
    pub id: String,
    pub kind: String,
    pub driver: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverSelection {
    pub id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresenterSelection {
    pub id: String,
    pub implementation: String,
    pub interactive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPolicy {
    pub authority_profile: String,
    pub trust_profile: String,
    pub update_profile: String,
    pub ambient_defaults: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostBounds {
    pub static_memory_bytes: u64,
    pub heap_arena_bytes: u64,
    pub queue_items: u32,
    pub buffered_bytes: u64,
    pub active_instances: u32,
    pub operation_slots: u32,
    pub timer_slots: u32,
    pub line_sessions: u32,
    pub evidence_items: u32,
}
