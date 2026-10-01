use crate::{PortId, ResourceClassId};
use serde::{Deserialize, Serialize};

/// Semantic contract for a Fore port carrying unforgeable resource authority.
///
/// This is descriptive checked/Plan truth. The authority value itself remains
/// an opaque [`crate::BaseCapabilityHandle`] owned by the local capability
/// table and is never represented by this serializable contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourcePortContract {
    pub port_id: PortId,
    pub class_id: ResourceClassId,
    pub ownership: ResourcePortOwnership,
    pub lifecycle: ResourcePortLifecycle,
    #[serde(default)]
    pub mobility: ResourcePortMobility,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourcePortOwnership {
    /// Acceptance moves the one admitted possession to the consumer.
    Move,
    /// The issuer explicitly admits shared possession under one capability.
    Shared,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourcePortLifecycle {
    Play,
    Plan,
    Boot,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourcePortMobility {
    /// Authority remains on the Host and Boot whose capability table issued it.
    #[default]
    HostLocal,
    /// Reserved for a resource class whose issuer provides an admitted transfer
    /// mechanism. Descriptive Line transport alone never satisfies this law.
    AdmittedTransfer,
}
