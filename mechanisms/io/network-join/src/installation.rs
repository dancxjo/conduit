//! Retained initialized network ownership before planning. No radio is inferred
//! from a descriptor, a request, serialized metadata or a successful probe.
use crate::{
    AssignedJoinSelection, JoinCallRefusal, NetworkJoinHostCall, NetworkJoinProvider,
    PreparedJoinContract,
};
use alloc::vec;
use conduit_core::*;
use conduit_net::{
    network_join_offer, NETWORK_CONFIG_AUTHORITY, NETWORK_CONFIG_SUBJECT,
    NETWORK_JOIN_EXECUTION_PROFILE, NETWORK_JOIN_HOST_CALL, WIFI_STATION_RESOURCE_CLASS,
};
use conduit_plan_lowering::{
    selected_assigned_operation::{AssignedNativeIdentity, SelectedAssignedOperationPlan},
    selected_operation::SelectedOperationPlan,
};
pub const JOIN_CAPABILITY: &str = "conduit/network-join@1";
pub const JOIN_BASE_KIND: &str = "machine/wifi-station";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkNativeIdentity {
    pub host: HostId,
    pub boot: BootId,
    pub base: HostBaseId,
    pub instance: BaseInstanceId,
    pub provider_generation: u64,
    pub resource: ResourcePoolId,
    pub resource_generation: ResourceGenerationId,
    pub envelope: CapabilityEnvelopeId,
    pub artifact: ArtifactId,
    pub implementation: ImplementationId,
    pub base_implementation: BaseImplementationId,
}
pub struct ReadyNetworkJoinBase<P> {
    identity: NetworkNativeIdentity,
    attachment_generation: u64,
    provider: P,
}
#[derive(Debug, PartialEq, Eq)]
pub enum NetworkInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}
impl<P: NetworkJoinProvider> ReadyNetworkJoinBase<P> {
    /// # Safety
    /// Root must exclusively retain this actual initialized provider, its
    /// controller/interfaces/services and current resource generation. Native SDK
    /// memory/work and cleanup must already have independent finite admission.
    /// The reserved attachment generation is not a successful attachment yet.
    /// Root must retain this owner while projecting availability and withdraw
    /// that projection before disposal. Metadata alone cannot satisfy these
    /// initialization, availability-lifetime and ownership facts.
    pub unsafe fn new(
        identity: NetworkNativeIdentity,
        attachment_generation: u64,
        provider: P,
    ) -> Result<Self, NetworkInstallationRefusal> {
        if [
            identity.host.as_str(),
            identity.boot.as_str(),
            identity.base.as_str(),
            identity.instance.as_str(),
            identity.resource.as_str(),
            &identity.resource_generation.0,
            identity.envelope.as_str(),
            identity.artifact.as_str(),
            identity.implementation.as_str(),
            identity.base_implementation.as_str(),
        ]
        .iter()
        .any(|s| s.is_empty())
            || identity.provider_generation == 0
            || attachment_generation == 0
        {
            return Err(NetworkInstallationRefusal::InvalidIdentity);
        }
        Ok(Self {
            identity,
            attachment_generation,
            provider,
        })
    }
    /// Add exactly the retained provider to the matching current Host/Boot.
    /// Preparation may allocate; this performs no radio configuration or join.
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
    ) -> Result<(), NetworkInstallationRefusal> {
        use NetworkInstallationRefusal as E;
        let i = &self.identity;
        if advertisement.host_id != i.host || advertisement.boot_id != i.boot {
            return Err(E::WrongBoot);
        }
        if advertisement
            .bases
            .iter()
            .any(|b| b.base_id == i.base || b.provider_instance_id == i.instance)
            || advertisement
                .capabilities
                .iter()
                .any(|c| c.capability_id.as_str() == JOIN_CAPABILITY)
            || advertisement
                .resources
                .iter()
                .any(|r| r.pool_id == i.resource)
        {
            return Err(E::Advertisement);
        }
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(
                advertisement.capabilities.len().saturating_add(1),
            )
            .map_err(|_| E::Advertisement)?,
            maximum_advertised_resources: u16::try_from(
                advertisement.resources.len().saturating_add(1),
            )
            .map_err(|_| E::Advertisement)?,
        })
        .map_err(|_| E::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: i.base.clone(),
                provider_instance_id: i.instance.clone(),
                provider_generation: i.provider_generation,
                implementation_id: i.base_implementation.clone(),
                mechanism_family: JOIN_BASE_KIND.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![network_join_offer(
                    JOIN_CAPABILITY.into(),
                    i.implementation.clone(),
                    i.artifact.clone(),
                )],
                resources: vec![resource_offer(
                    i.resource.as_str(),
                    WIFI_STATION_RESOURCE_CLASS,
                    1,
                )],
            })
            .map_err(|_| E::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| E::Advertisement)
    }
    fn claim_matches(&self, claim: &BaseOperationClaim) -> bool {
        let i = &self.identity;
        claim.host_id == i.host
            && claim.boot_id == i.boot
            && claim.base_instance_id == i.instance
            && claim.base_provider_generation == i.provider_generation
            && claim.resource_pool_id == i.resource
            && claim.resource_generation_id == i.resource_generation
            && claim.envelope_id == i.envelope
            && claim.implementation_id == i.implementation
    }
    /// Consume retained native ownership into the exact selected Plan/Play.
    pub fn bind_selected(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PreparedJoinContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<NetworkJoinHostCall<P>, JoinCallRefusal> {
        let gear = selected
            .fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selected.placement_id)
            .ok_or(JoinCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(JoinCallRefusal::WrongBinding)?;
        let i = &self.identity;
        if !self.claim_matches(&claim)
            || gear.artifact_id != i.artifact
            || gear.implementation_id != i.implementation
            || gear.execution_profile_id.as_str() != NETWORK_JOIN_EXECUTION_PROFILE
            || gear.capability_id.as_str() != JOIN_CAPABILITY
            || gear.limits != contract.kind().limits
            || base.base_id != i.base
            || base.provider_instance_id != i.instance
            || base.provider_generation != i.provider_generation
            || base.implementation_id != i.base_implementation
            || base.mechanism_family.as_str() != JOIN_BASE_KIND
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || !gear.authority.iter().any(|a| {
                a.contract_id.as_str() == NETWORK_CONFIG_AUTHORITY
                    && a.host_call_contract_id.as_str() == NETWORK_JOIN_HOST_CALL
                    && a.subject_kind.as_str() == NETWORK_CONFIG_SUBJECT
            })
        {
            return Err(JoinCallRefusal::WrongBinding);
        }
        // SAFETY: new retained the real provider. The exact native identity above
        // preserves its initialization, resource generation and ownership scope.
        unsafe {
            NetworkJoinHostCall::bind_selected(
                table,
                handle,
                claim,
                self.provider,
                self.attachment_generation,
                contract,
                selected,
            )
        }
    }
    /// Root supplies only independently admitted assignment/Play facts. The
    /// implementation, artifact, Base and resource identities come from this owner.
    pub fn bind_assigned(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PreparedJoinContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<NetworkJoinHostCall<P>, JoinCallRefusal> {
        if !self.claim_matches(&claim) {
            return Err(JoinCallRefusal::WrongBinding);
        }
        let i = &self.identity;
        let native = AssignedNativeIdentity {
            capability: JOIN_CAPABILITY,
            execution_profile: NETWORK_JOIN_EXECUTION_PROFILE,
            artifact: i.artifact.as_str(),
            base: i.base.as_str(),
            base_implementation: i.base_implementation.as_str(),
            mechanism: JOIN_BASE_KIND,
            authority: NETWORK_CONFIG_AUTHORITY,
            resource_class: WIFI_STATION_RESOURCE_CLASS,
        };
        // SAFETY: retained initialization and exact local native identity above;
        // the common binder checks assignment facts and opaque possession.
        unsafe {
            NetworkJoinHostCall::bind_assigned(
                table,
                handle,
                claim,
                self.provider,
                self.attachment_generation,
                AssignedJoinSelection {
                    contract,
                    native,
                    plan: selected,
                },
            )
        }
    }
}
