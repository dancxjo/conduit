//! Ordinary native installation of the separately admitted fixed frame queues.
//! No USB identity, class meaning, network discovery or generated authority.
use super::{
    contract::*,
    factory::*,
    network_session::*,
    offer,
    owner::{EthernetFrameCallOwner, FrameCallRefusal},
    queues::EthernetFrameQueues,
};
use crate::machine_membrane::selected_operation::SelectedOperationPlan;
use alloc::vec;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;
use smoltcp::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EthernetFrameNativeIdentity {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub base_id: HostBaseId,
    pub provider_instance_id: BaseInstanceId,
    pub provider_generation: u64,
    pub resource_pool_id: ResourcePoolId,
    pub resource_generation_id: ResourceGenerationId,
    pub envelope_id: CapabilityEnvelopeId,
    pub artifact_id: ArtifactId,
}
#[derive(Debug)]
pub enum EthernetFrameNativeRefusal {
    Identity,
    Advertisement,
    Binding,
    Retention,
    Possession,
    StaleRequest,
    Capability(BaseCapabilityRefusal),
    Canonical(StructuredInfoRefusal),
    Network(EthernetNetworkRefusal),
}
impl From<FrameCallRefusal> for EthernetFrameNativeRefusal {
    fn from(error: FrameCallRefusal) -> Self {
        match error {
            FrameCallRefusal::Binding => Self::Binding,
            FrameCallRefusal::Possession => Self::Possession,
            FrameCallRefusal::StaleRequest => Self::StaleRequest,
            FrameCallRefusal::Capability(e) => Self::Capability(e),
            FrameCallRefusal::Canonical(e) => Self::Canonical(e),
        }
    }
}
pub struct EthernetFrameSelection<'a> {
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement: &'a PlacementId,
}

pub struct ReadyEthernetFrameBase<'a> {
    identity: EthernetFrameNativeIdentity,
    queues: &'a EthernetFrameQueues,
    approval: EthernetNetworkApproval,
}
impl<'a> ReadyEthernetFrameBase<'a> {
    /// # Safety
    /// Root owns these exact stable fixed queues under this independently
    /// admitted network Base/resource generation/envelope. No competing stack
    /// or Call owner accesses them; storage remains valid until Play stops.
    /// Approval came from actual administrator authorization, not USB discovery.
    pub unsafe fn new(
        identity: EthernetFrameNativeIdentity,
        queues: &'a EthernetFrameQueues,
        approval: EthernetNetworkApproval,
    ) -> Result<Self, EthernetFrameNativeRefusal> {
        if identity.host_id.as_str().is_empty()
            || identity.boot_id.as_str().is_empty()
            || identity.base_id.as_str().is_empty()
            || identity.provider_instance_id.as_str().is_empty()
            || identity.resource_pool_id.as_str().is_empty()
            || identity.resource_generation_id.0.is_empty()
            || identity.envelope_id.as_str().is_empty()
            || identity.artifact_id.as_str().is_empty()
            || identity.provider_generation != queues.generation()
            || queues.generation() == 0
            || approval.scope().mac != queues.mac()
            || approval.host_id() != &identity.host_id
            || approval.boot_id() != &identity.boot_id
        {
            return Err(EthernetFrameNativeRefusal::Identity);
        }
        queues
            .transmit_ready(identity.provider_generation)
            .map_err(|error| {
                EthernetFrameNativeRefusal::Network(EthernetNetworkRefusal::Queue(error))
            })?;
        Ok(Self {
            identity,
            queues,
            approval,
        })
    }
    pub fn identity(&self) -> &EthernetFrameNativeIdentity {
        &self.identity
    }
    pub fn scope(&self) -> EthernetNetworkScope {
        self.approval.scope()
    }
    pub fn resource_bytes(&self) -> u64 {
        core::mem::size_of::<EthernetFrameQueues>() as u64
    }
    pub(crate) fn retained_identity_bytes(&self) -> Option<usize> {
        let id = &self.identity;
        let mut bytes = core::mem::size_of::<Self>();
        for text in [
            &id.host_id.0,
            &id.boot_id.0,
            &id.base_id.0,
            &id.provider_instance_id.0,
            &id.resource_pool_id.0,
            &id.resource_generation_id.0,
            &id.envelope_id.0,
            &id.artifact_id.0,
            &self.approval.host_id().0,
            &self.approval.boot_id().0,
        ] {
            bytes = bytes.checked_add(super::retention::rounded(text.capacity())?)?;
        }
        Some(bytes)
    }
    /// Inert description of the actual initialized queue provider, not possession.
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &EthernetFrameContract,
    ) -> Result<(), EthernetFrameNativeRefusal> {
        let id = &self.identity;
        if advertisement.host_id != id.host_id
            || advertisement.boot_id != id.boot_id
            || advertisement.bases.iter().any(|base| {
                base.base_id == id.base_id || base.provider_instance_id == id.provider_instance_id
            })
            || advertisement
                .resources
                .iter()
                .any(|r| r.pool_id == id.resource_pool_id)
            || advertisement
                .capabilities
                .iter()
                .any(|c| c.capability_id.as_str() == FRAME_IMPLEMENTATION)
        {
            return Err(EthernetFrameNativeRefusal::Advertisement);
        }
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(advertisement.capabilities.len() + 1)
                .map_err(|_| EthernetFrameNativeRefusal::Advertisement)?,
            maximum_advertised_resources: u16::try_from(advertisement.resources.len() + 1)
                .map_err(|_| EthernetFrameNativeRefusal::Advertisement)?,
        })
        .map_err(|_| EthernetFrameNativeRefusal::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: id.base_id.clone(),
                provider_instance_id: id.provider_instance_id.clone(),
                provider_generation: id.provider_generation,
                implementation_id: FRAME_BASE.into(),
                mechanism_family: FRAME_ATTACHMENT.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer::offer(contract, id.artifact_id.clone())],
                resources: vec![resource_offer(
                    id.resource_pool_id.as_str(),
                    FRAME_ATTACHMENT,
                    1,
                )],
            })
            .map_err(|_| EthernetFrameNativeRefusal::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| EthernetFrameNativeRefusal::Advertisement)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn bind_selected(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &EthernetFrameContract,
        selection: EthernetFrameSelection<'_>,
        storage: EthernetNetworkStorage<'a>,
        random_seed: u64,
        timestamp: Instant,
    ) -> Result<NativeEthernetFrameHost<'a>, EthernetFrameNativeRefusal> {
        let id = &self.identity;
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selection.placement)
            .ok_or(EthernetFrameNativeRefusal::Binding)?;
        let base = gear
            .base
            .as_ref()
            .ok_or(EthernetFrameNativeRefusal::Binding)?;
        if gear.implementation_id.as_str() != FRAME_IMPLEMENTATION
            || gear.capability_id.as_str() != FRAME_IMPLEMENTATION
            || gear.execution_profile_id.as_str() != FRAME_PROFILE
            || claim.implementation_id.as_str() != FRAME_IMPLEMENTATION
            || claim.host_id != id.host_id
            || claim.boot_id != id.boot_id
            || claim.base_instance_id != id.provider_instance_id
            || claim.base_provider_generation != id.provider_generation
            || claim.resource_pool_id != id.resource_pool_id
            || claim.resource_generation_id != id.resource_generation_id
            || claim.envelope_id != id.envelope_id
            || gear.artifact_id != id.artifact_id
            || base.base_id != id.base_id
            || base.provider_instance_id != id.provider_instance_id
            || base.provider_generation != id.provider_generation
            || base.implementation_id.as_str() != FRAME_BASE
            || base.mechanism_family.as_str() != FRAME_ATTACHMENT
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(EthernetFrameNativeRefusal::Binding);
        }
        // SAFETY: new retained exact Root ownership; identity and ordinary
        // selection validation bind its independent cryptographic possession.
        let calls = unsafe {
            EthernetFrameCallOwner::bind_admitted(
                self.queues,
                table,
                handle,
                claim,
                contract,
                SelectedOperationPlan {
                    fragment: selection.fragment,
                    lowered: selection.lowered,
                    active: selection.active,
                    placement_id: selection.placement,
                },
            )
        }
        .map_err(EthernetFrameNativeRefusal::from)?;
        let network = PreparedEthernetNetwork::prepare(
            self.queues,
            self.approval,
            storage,
            random_seed,
            timestamp,
        )
        .map_err(EthernetFrameNativeRefusal::Network)?;
        Ok(NativeEthernetFrameHost {
            calls,
            network,
            cancelled: false,
            stopping: false,
        })
    }
}

/// One queue owner and one existing stack session share the same actual storage.
/// Root invokes these hooks from the same ordinary retained Body/Play lifecycle.
pub struct NativeEthernetFrameHost<'a> {
    calls: EthernetFrameCallOwner<'a>,
    network: PreparedEthernetNetwork<'a>,
    cancelled: bool,
    stopping: bool,
}
impl NativeEthernetFrameHost<'_> {
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], EthernetFrameNativeRefusal> {
        if self.stopping && !self.cancelled {
            return Err(EthernetFrameNativeRefusal::Binding);
        }
        if self.cancelled {
            return Err(EthernetFrameNativeRefusal::Capability(
                BaseCapabilityRefusal::Revoked,
            ));
        }
        self.calls
            .invoke(node, call, request, input)
            .map_err(EthernetFrameNativeRefusal::from)
    }
    pub fn poll_network(
        &mut self,
        timestamp: Instant,
    ) -> Result<Option<crate::virtio_tcp::VirtioTcpReceipt>, EthernetFrameNativeRefusal> {
        if self.stopping {
            return Err(EthernetFrameNativeRefusal::Binding);
        }
        self.network
            .poll_one(timestamp)
            .map_err(EthernetFrameNativeRefusal::Network)
    }
    pub fn received(&self) -> &[u8] {
        self.network.received()
    }
    pub fn polls(&self) -> u32 {
        self.network.polls()
    }
    pub fn scope(&self) -> EthernetNetworkScope {
        self.network.scope()
    }
    pub fn cancel(&mut self) -> Result<(), EthernetFrameNativeRefusal> {
        if self.cancelled {
            return Ok(());
        }
        self.stopping = true;
        self.network.cancel();
        self.calls
            .cancel()
            .map_err(EthernetFrameNativeRefusal::from)?;
        self.cancelled = true;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
