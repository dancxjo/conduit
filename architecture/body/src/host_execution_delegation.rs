//! Explicit construction intent for Root's execution decision, not a grant.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostExecutionAuthorizer {
    SeedOwner,
}
impl HostExecutionAuthorizer {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SeedOwner => "seed-owner",
        }
    }
}

/// Root may consider starting exact work approved by this selected issuer.
/// Preparation delegation is independently required. This ceiling adds no work,
/// storage, native capability, input authority or resource reservation. Root
/// must validate the complete current workset and acquire its actual resources
/// and local possessions before transferring providers or starting any kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExecutionDelegation {
    pub authorizer: HostExecutionAuthorizer,
    pub maximum_active_worksets: u16,
    /// Separate opt-in for owner-signed cross-Boot recipe permission. Absence
    /// preserves current-Boot-only execution. No ceiling supplies a grant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation: Option<crate::ContinuationCeilings>,
}
