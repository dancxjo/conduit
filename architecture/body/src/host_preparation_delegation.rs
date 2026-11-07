//! Installed construction intent for delegated preparation, not an effect grant.
use serde::{Deserialize, Serialize};

/// Control request on an already selected Host carrier; it grants no authority.
pub const HOST_PREPARATION_REQUEST: &[u8] = b"conduit.body/prepare-fragments@1";

/// The Seed resolves this explicit selection to its independently pinned owner.
/// Merely having an owner or joining its Body does not select this policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostPreparationAuthorizer {
    SeedOwner,
}
impl HostPreparationAuthorizer {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SeedOwner => "seed-owner",
        }
    }
}

/// Permission to consider an approved exact Body-fragment preparation request.
/// These are policy ceilings, never enlarged memory or native capability grants.
/// Root still checks the exact approved projection, reserves actual resources,
/// obtains local possession, and starts/controls the one kernel independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPreparationDelegation {
    pub authorizer: HostPreparationAuthorizer,
    pub maximum_assignment_bytes: u32,
    pub maximum_runtime_state_bytes: u32,
}
impl HostPreparationDelegation {
    pub const fn has_finite_bounds(&self) -> bool {
        self.maximum_assignment_bytes != 0 && self.maximum_runtime_state_bytes != 0
    }
}

#[cfg(feature = "authenticated-admission")]
mod check;
#[cfg(feature = "authenticated-admission")]
pub use check::*;
