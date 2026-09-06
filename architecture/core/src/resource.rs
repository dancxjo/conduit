use crate::{
    ArchitectureBaseId, BaseExecutionLaneId, BootId, CapabilityId, ComputeDomainId,
    ComputePerformanceClassId, ComputeTopologyGroupId, GearId, HostId, OfferGeneration,
    ResourceAllowanceSourceId, ResourceBindingRoleId, ResourceClassId, ResourceHandleId,
    ResourcePoolId, SignId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceRequirement {
    pub class_id: ResourceClassId,
    pub units: u32,
    #[serde(default)]
    pub protected_role: Option<ResourceBindingRoleId>,
    #[serde(default)]
    pub compute: Option<ComputeRequirement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<crate::ResourceContentRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceOffer {
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub capacity_units: u32,
    #[serde(default)]
    pub compute: Option<ComputePoolContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<crate::ResourceContentOffer>,
}

/// Body-neutral planning ceiling over one unchanged Host resource pool.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ResourceAllowance {
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub maximum_units: u32,
}

/// Attributable, boot-exact resource constraints supplied to generic planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceAllowanceSet {
    pub source_id: ResourceAllowanceSourceId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub allowances: alloc::vec::Vec<ResourceAllowance>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ComputeServiceGuarantee {
    Shared,
    Reserved,
    Exclusive,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ArchitectureBaseKind {
    HostedOs,
    BareMetal,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ComputeTopologyGroup {
    pub group_id: ComputeTopologyGroupId,
    pub lane_capacity: u32,
    pub numa_domain: Option<ComputeDomainId>,
    pub cache_domain: Option<ComputeDomainId>,
    pub performance_class: Option<ComputePerformanceClassId>,
    /// Stable scheduler-visible nominal clock when the Base can truthfully
    /// expose it. Current measured throughput and thermal state remain Signs.
    pub nominal_clock_hz: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ComputePoolContract {
    pub service_guarantee: ComputeServiceGuarantee,
    pub architecture_base_id: ArchitectureBaseId,
    pub architecture_base_kind: ArchitectureBaseKind,
    /// Optional truthful topology groups. Empty means topology is unknown or
    /// not contractually exposed, not that the machine has no topology.
    pub topology_groups: alloc::vec::Vec<ComputeTopologyGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ComputeTopologyRequirement {
    pub same_numa_domain: bool,
    pub same_cache_domain: bool,
    pub performance_class: Option<ComputePerformanceClassId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ComputeRequirement {
    pub minimum_lanes: u32,
    pub preferred_lanes: u32,
    pub maximum_lanes: u32,
    pub minimum_service_guarantee: ComputeServiceGuarantee,
    pub topology: Option<ComputeTopologyRequirement>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceHealth {
    Ready,
    Unavailable,
}

/// Mutable current sign about one stable, boot-scoped resource pool.
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
    pub sign_id: SignId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceBinding {
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub units: u32,
    #[serde(default)]
    pub protected: Option<ProtectedResourceBinding>,
    #[serde(default)]
    pub compute: Option<ComputeReservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<crate::ResourceContentOffer>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ComputeReservation {
    pub selected_lanes: u32,
    pub service_guarantee: ComputeServiceGuarantee,
    pub architecture_base_id: ArchitectureBaseId,
    pub architecture_base_kind: ArchitectureBaseKind,
    pub topology_group_id: Option<ComputeTopologyGroupId>,
    pub performance_class: Option<ComputePerformanceClassId>,
    pub nominal_clock_hz: Option<u64>,
}

/// Transient base/runtime fact, deliberately absent from Plan identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ComputeLaneAssignment {
    pub architecture_base_id: ArchitectureBaseId,
    pub base_lane_id: BaseExecutionLaneId,
    pub active_play_id: crate::ActivePlayId,
    pub placement_id: crate::PlacementId,
}

/// Architecture-neutral operation vocabulary for a bare-metal execution-lane
/// base. Hosted compositions may realize the same entitlement through OS
/// worker scheduling instead.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ArchitectureLaneOperation {
    EnumerateTopology,
    Start,
    Wake,
    Run,
    Park,
    Signal,
}

impl ComputeRequirement {
    pub fn is_valid_for_units(&self, units: u32) -> bool {
        self.minimum_lanes > 0
            && self.minimum_lanes == units
            && self.minimum_lanes <= self.preferred_lanes
            && self.preferred_lanes <= self.maximum_lanes
    }
}

impl ComputePoolContract {
    pub fn is_valid_for_capacity(&self, capacity_units: u32) -> bool {
        !self.architecture_base_id.as_str().is_empty()
            && self.topology_groups.iter().all(|group| {
                !group.group_id.as_str().is_empty()
                    && group.lane_capacity > 0
                    && group.lane_capacity <= capacity_units
                    && group
                        .numa_domain
                        .as_ref()
                        .is_none_or(|id| !id.as_str().is_empty())
                    && group
                        .cache_domain
                        .as_ref()
                        .is_none_or(|id| !id.as_str().is_empty())
                    && group
                        .performance_class
                        .as_ref()
                        .is_none_or(|id| !id.as_str().is_empty())
                    && group.nominal_clock_hz.is_none_or(|hz| hz > 0)
            })
            && self
                .topology_groups
                .windows(2)
                .all(|pair| pair[0].group_id < pair[1].group_id)
    }
}

pub fn compute_reservation(
    requirement: &ResourceRequirement,
    offer: &ResourceOffer,
    available_units: u32,
) -> Option<ComputeReservation> {
    let required = requirement.compute.as_ref()?;
    let contract = offer.compute.as_ref()?;
    if !required.is_valid_for_units(requirement.units)
        || !contract.is_valid_for_capacity(offer.capacity_units)
        || contract.service_guarantee < required.minimum_service_guarantee
        || available_units < required.minimum_lanes
    {
        return None;
    }
    let topology = match &required.topology {
        Some(topology) => Some(contract.topology_groups.iter().find(|group| {
            group.lane_capacity >= required.minimum_lanes
                && (!topology.same_numa_domain || group.numa_domain.is_some())
                && (!topology.same_cache_domain || group.cache_domain.is_some())
                && topology
                    .performance_class
                    .as_ref()
                    .is_none_or(|class| group.performance_class.as_ref() == Some(class))
        })?),
        None => None,
    };
    let topology_capacity = topology.map_or(available_units, |group| group.lane_capacity);
    let selected_lanes = required
        .preferred_lanes
        .min(required.maximum_lanes)
        .min(available_units)
        .min(topology_capacity);
    (selected_lanes >= required.minimum_lanes).then(|| ComputeReservation {
        selected_lanes,
        service_guarantee: contract.service_guarantee,
        architecture_base_id: contract.architecture_base_id.clone(),
        architecture_base_kind: contract.architecture_base_kind,
        topology_group_id: topology.map(|group| group.group_id.clone()),
        performance_class: topology.and_then(|group| group.performance_class.clone()),
        nominal_clock_hz: topology.and_then(|group| group.nominal_clock_hz),
    })
}

/// Verifies that a sealed binding is a valid realization of one advertised
/// requirement and pool. Scalable compute bindings intentionally need not be
/// textually equal to their minimum requirement.
pub fn resource_binding_satisfies(
    binding: &ResourceBinding,
    requirement: &ResourceRequirement,
    offer: &ResourceOffer,
) -> bool {
    if !crate::resource_content::content_binding_satisfies(binding, requirement, offer)
        || binding.pool_id != offer.pool_id
        || binding.class_id != requirement.class_id
        || offer.class_id != requirement.class_id
        || binding.protected.as_ref().map(|value| &value.role_id)
            != requirement.protected_role.as_ref()
    {
        return false;
    }
    match (&requirement.compute, &offer.compute, &binding.compute) {
        (None, None, None) => binding.units == requirement.units,
        (Some(required), Some(contract), Some(reservation)) => {
            required.is_valid_for_units(requirement.units)
                && contract.is_valid_for_capacity(offer.capacity_units)
                && binding.units == reservation.selected_lanes
                && binding.units >= required.minimum_lanes
                && binding.units <= required.maximum_lanes
                && binding.units <= offer.capacity_units
                && reservation.service_guarantee == contract.service_guarantee
                && reservation.service_guarantee >= required.minimum_service_guarantee
                && reservation.architecture_base_id == contract.architecture_base_id
                && reservation.architecture_base_kind == contract.architecture_base_kind
                && match &reservation.topology_group_id {
                    Some(group_id) => contract.topology_groups.iter().any(|group| {
                        &group.group_id == group_id
                            && group.performance_class == reservation.performance_class
                            && group.nominal_clock_hz == reservation.nominal_clock_hz
                    }),
                    None => {
                        reservation.performance_class.is_none()
                            && reservation.nominal_clock_hz.is_none()
                    }
                }
                && match (&required.topology, &reservation.topology_group_id) {
                    (None, None) => true,
                    (Some(topology), Some(group_id)) => {
                        contract.topology_groups.iter().any(|group| {
                            &group.group_id == group_id
                                && group.lane_capacity >= binding.units
                                && (!topology.same_numa_domain || group.numa_domain.is_some())
                                && (!topology.same_cache_domain || group.cache_domain.is_some())
                                && topology.performance_class.as_ref().is_none_or(|class| {
                                    group.performance_class.as_ref() == Some(class)
                                })
                        })
                    }
                    _ => false,
                }
        }
        _ => false,
    }
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
    /// Opaque, single-issuance identity. A Host must never reissue a revoked
    /// handle to make an older Plan current again.
    pub handle_id: ResourceHandleId,
    pub gear_id: GearId,
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
        content: None,
        class_id: ResourceClassId::from(class_id),
        units,
        protected_role: None,
        compute: None,
    }
}

pub fn protected_resource_requirement(
    role_id: &str,
    class_id: &str,
    units: u32,
) -> ResourceRequirement {
    ResourceRequirement {
        content: None,
        class_id: ResourceClassId::from(class_id),
        units,
        protected_role: Some(ResourceBindingRoleId::from(role_id)),
        compute: None,
    }
}

pub fn resource_offer(pool_id: &str, class_id: &str, capacity_units: u32) -> ResourceOffer {
    ResourceOffer {
        content: None,
        pool_id: ResourcePoolId::from(pool_id),
        class_id: ResourceClassId::from(class_id),
        capacity_units,
        compute: None,
    }
}

pub fn compute_resource_requirement(
    class_id: &str,
    minimum_lanes: u32,
    preferred_lanes: u32,
    maximum_lanes: u32,
    minimum_service_guarantee: ComputeServiceGuarantee,
    topology: Option<ComputeTopologyRequirement>,
) -> ResourceRequirement {
    ResourceRequirement {
        content: None,
        class_id: ResourceClassId::from(class_id),
        units: minimum_lanes,
        protected_role: None,
        compute: Some(ComputeRequirement {
            minimum_lanes,
            preferred_lanes,
            maximum_lanes,
            minimum_service_guarantee,
            topology,
        }),
    }
}

pub fn compute_resource_offer(
    pool_id: &str,
    class_id: &str,
    capacity_units: u32,
    contract: ComputePoolContract,
) -> ResourceOffer {
    ResourceOffer {
        content: None,
        pool_id: ResourcePoolId::from(pool_id),
        class_id: ResourceClassId::from(class_id),
        capacity_units,
        compute: Some(contract),
    }
}
