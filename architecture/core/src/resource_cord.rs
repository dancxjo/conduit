//! Runtime possession transfer for an exact local resource Cord.
//!
//! Plans carry only descriptive resource provenance. This module keeps the
//! unforgeable bearer outside serialization and moves it exactly once from the
//! planned source endpoint to the planned sink endpoint.

use crate::{
    ActivePlayId, BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable,
    BaseOperationClaim, BaseOperationLease, ConnectionId, PlacementId, PlanId, PlannedConnection,
    PortId, ResourceBinding, ResourcePortContract, ResourcePortLifecycle, ResourcePortMobility,
    ResourcePortOwnership,
};
use alloc::boxed::Box;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceCordRefusal {
    NotResourceCord,
    WrongOwner,
    UnsupportedMobility,
    UnsupportedOwnership,
    WrongSink,
    WrongLifecycle,
    Capability(BaseCapabilityRefusal),
}

/// One offered move-only possession awaiting acceptance at the exact Plan sink.
#[derive(Debug)]
pub struct OfferedResourceCord {
    plan_id: PlanId,
    active_play_id: ActivePlayId,
    connection_id: ConnectionId,
    source_placement_id: PlacementId,
    sink_placement_id: PlacementId,
    sink_port_id: PortId,
    contract: ResourcePortContract,
    binding: ResourceBinding,
    handle: BaseCapabilityHandle,
}

/// One accepted local possession. It can be exercised only through its issuer
/// table and is explicitly revoked at its Plan-declared lifecycle boundary.
#[derive(Debug)]
pub struct AcceptedResourceCord {
    plan_id: PlanId,
    active_play_id: ActivePlayId,
    connection_id: ConnectionId,
    source_placement_id: PlacementId,
    sink_placement_id: PlacementId,
    sink_port_id: PortId,
    contract: ResourcePortContract,
    binding: ResourceBinding,
    handle: BaseCapabilityHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCordInspection {
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub connection_id: ConnectionId,
    pub source_placement_id: PlacementId,
    pub sink_placement_id: PlacementId,
    pub sink_port_id: PortId,
    pub contract: ResourcePortContract,
    pub binding: ResourceBinding,
}

impl OfferedResourceCord {
    pub fn new(
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        connection: &PlannedConnection,
        handle: BaseCapabilityHandle,
    ) -> Result<Self, ResourceCordRefusal> {
        let resource = connection
            .resource
            .as_ref()
            .ok_or(ResourceCordRefusal::NotResourceCord)?;
        if resource.owner_placement_id != connection.source_placement_id
            || resource.contract.port_id != connection.source_port_id
        {
            return Err(ResourceCordRefusal::WrongOwner);
        }
        if resource.contract.mobility != ResourcePortMobility::HostLocal {
            return Err(ResourceCordRefusal::UnsupportedMobility);
        }
        if resource.contract.ownership != ResourcePortOwnership::Move {
            return Err(ResourceCordRefusal::UnsupportedOwnership);
        }
        Ok(Self {
            plan_id,
            active_play_id,
            connection_id: connection.connection_id.clone(),
            source_placement_id: connection.source_placement_id.clone(),
            sink_placement_id: connection.sink_placement_id.clone(),
            sink_port_id: connection.sink_port_id.clone(),
            contract: resource.contract.clone(),
            binding: resource.source_binding.clone(),
            handle,
        })
    }

    pub fn source_placement_id(&self) -> &PlacementId {
        &self.source_placement_id
    }

    pub fn accept(
        self,
        sink_placement_id: &PlacementId,
        sink_port_id: &PortId,
    ) -> Result<AcceptedResourceCord, Box<(ResourceCordRefusal, Self)>> {
        if sink_placement_id != &self.sink_placement_id || sink_port_id != &self.sink_port_id {
            return Err(Box::new((ResourceCordRefusal::WrongSink, self)));
        }
        Ok(AcceptedResourceCord {
            plan_id: self.plan_id,
            active_play_id: self.active_play_id,
            connection_id: self.connection_id,
            source_placement_id: self.source_placement_id,
            sink_placement_id: self.sink_placement_id,
            sink_port_id: self.sink_port_id,
            contract: self.contract,
            binding: self.binding,
            handle: self.handle,
        })
    }
}

impl AcceptedResourceCord {
    pub fn inspection(&self) -> ResourceCordInspection {
        ResourceCordInspection {
            plan_id: self.plan_id.clone(),
            active_play_id: self.active_play_id.clone(),
            connection_id: self.connection_id.clone(),
            source_placement_id: self.source_placement_id.clone(),
            sink_placement_id: self.sink_placement_id.clone(),
            sink_port_id: self.sink_port_id.clone(),
            contract: self.contract.clone(),
            binding: self.binding.clone(),
        }
    }

    pub fn authorize(
        &mut self,
        table: &mut BaseCapabilityTable,
        claim: &BaseOperationClaim,
    ) -> Result<BaseOperationLease, ResourceCordRefusal> {
        table
            .authorize(&self.handle, claim)
            .map_err(ResourceCordRefusal::Capability)
    }

    pub fn complete(
        &mut self,
        table: &mut BaseCapabilityTable,
        lease: BaseOperationLease,
        result_bytes: u32,
    ) -> Result<(), ResourceCordRefusal> {
        table
            .complete(&mut self.handle, lease, result_bytes)
            .map_err(ResourceCordRefusal::Capability)
    }

    pub fn retire(
        self,
        table: &mut BaseCapabilityTable,
        boundary: ResourcePortLifecycle,
    ) -> Result<ResourceCordInspection, ResourceCordRefusal> {
        if boundary != self.contract.lifecycle {
            return Err(ResourceCordRefusal::WrongLifecycle);
        }
        table
            .revoke(&self.handle)
            .map_err(ResourceCordRefusal::Capability)?;
        Ok(self.inspection())
    }
}
