use crate::{
    BootId, CapabilityId, EvidenceId, HostId, OfferGeneration, OperationId, ResourceBindingRoleId,
    ResourceClassId, ResourceHandleId, ResourcePoolId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceRequirement {
    pub class_id: ResourceClassId,
    pub units: u32,
    #[serde(default)]
    pub protected_role: Option<ResourceBindingRoleId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceOffer {
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub capacity_units: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceHealth {
    Ready,
    Unavailable,
}

/// Mutable current evidence about one stable, boot-scoped resource pool.
///
/// Unreserved capacity, current utilization, and concrete scheduler lane
/// assignment are distinct. This observation deliberately contains no lane or
/// physical processor identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceObservation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub health: ResourceHealth,
    pub unreserved_units: u32,
    pub utilized_units: u32,
    pub evidence_id: EvidenceId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceBinding {
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub units: u32,
    #[serde(default)]
    pub protected: Option<ProtectedResourceBinding>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProtectedResourceAccess {
    ReadExisting,
    Create,
    Replace,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProtectedResourceCommitPolicy {
    NotApplicable,
    CreateOnly,
    ReplaceExisting,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtectedResourceGrant {
    pub role_id: ResourceBindingRoleId,
    pub handle_id: ResourceHandleId,
    pub operation_id: OperationId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub class_id: ResourceClassId,
    pub access: ProtectedResourceAccess,
    pub maximum_bytes: u64,
    pub commit_policy: ProtectedResourceCommitPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtectedResourceBinding {
    pub role_id: ResourceBindingRoleId,
    pub handle_id: ResourceHandleId,
    pub access: ProtectedResourceAccess,
    pub maximum_bytes: u64,
    pub commit_policy: ProtectedResourceCommitPolicy,
}

pub fn resource_requirement(class_id: &str, units: u32) -> ResourceRequirement {
    ResourceRequirement {
        class_id: ResourceClassId::from(class_id),
        units,
        protected_role: None,
    }
}

pub fn protected_resource_requirement(
    role_id: &str,
    class_id: &str,
    units: u32,
) -> ResourceRequirement {
    ResourceRequirement {
        class_id: ResourceClassId::from(class_id),
        units,
        protected_role: Some(ResourceBindingRoleId::from(role_id)),
    }
}

pub fn resource_offer(pool_id: &str, class_id: &str, capacity_units: u32) -> ResourceOffer {
    ResourceOffer {
        pool_id: ResourcePoolId::from(pool_id),
        class_id: ResourceClassId::from(class_id),
        capacity_units,
    }
}
