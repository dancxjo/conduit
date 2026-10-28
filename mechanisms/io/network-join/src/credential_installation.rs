//! Initialized private credential-source ownership; never inferred from metadata.
use crate::installation::{NetworkInstallationRefusal, NetworkNativeIdentity};
use crate::{
    CredentialCallRefusal, CredentialSource, NetworkCredentialsHostCall,
    PreparedCredentialContract, CREDENTIAL_RESOURCE_CLASS,
};
use alloc::vec;
use conduit_core::*;
use conduit_net::*;
use conduit_plan_lowering::selected_assigned_operation::{
    AssignedNativeIdentity, SelectedAssignedOperationPlan,
};
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
pub const CREDENTIAL_CAPABILITY: &str = "conduit/network-credentials@1";
pub const CREDENTIAL_BASE_KIND: &str = "machine/private-credentials";
const CREDENTIAL_EXECUTION_PROFILE: &str = "conduit.network/credentials-hosted@1";
pub struct ReadyCredentialBase<P> {
    identity: NetworkNativeIdentity,
    source: P,
}
impl<P: CredentialSource> ReadyCredentialBase<P> {
    /// # Safety
    /// Root must exclusively retain the actual initialized volatile source,
    /// reserve its finite storage/work, and own its exact current identity and
    /// resource generation. Retain this owner while advertised and withdraw the
    /// projection before disposal. Metadata and a successful probe are insufficient.
    pub unsafe fn new(
        identity: NetworkNativeIdentity,
        source: P,
    ) -> Result<Self, NetworkInstallationRefusal> {
        let i = &identity;
        if [
            i.host.as_str(),
            i.boot.as_str(),
            i.base.as_str(),
            i.instance.as_str(),
            i.resource.as_str(),
            &i.resource_generation.0,
            i.envelope.as_str(),
            i.artifact.as_str(),
            i.implementation.as_str(),
            i.base_implementation.as_str(),
        ]
        .iter()
        .any(|s| s.is_empty())
            || i.provider_generation == 0
        {
            return Err(NetworkInstallationRefusal::InvalidIdentity);
        }
        Ok(Self { identity, source })
    }
    /// Descriptive native identity retained with this owner; not authority or
    /// evidence of possession, and never reconstructed from a received Plan.
    pub fn native_identity(&self) -> &NetworkNativeIdentity {
        &self.identity
    }
    /// Return the actual initialized, unbound owner for Root preparation.
    /// This consumes readiness; it never extracts a source from an installed
    /// Host Call or reconstructs possession from advertisement metadata. Root
    /// must withdraw the projection while preparing and retain this same native
    /// identity, source, generations and finite counters when reissuing readiness.
    /// Preparing private input grants no credential invocation or radio effect.
    pub fn into_unbound_parts(self) -> (NetworkNativeIdentity, P) {
        (self.identity, self.source)
    }
    pub(crate) fn into_checked_call(
        self,
        operation: conduit_core::BoundBaseOperation,
        node: conduit_kernel::NodeId,
    ) -> NetworkCredentialsHostCall<P> {
        NetworkCredentialsHostCall::bound_operation(operation, self.source, node)
    }
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
                .any(|c| c.capability_id.as_str() == CREDENTIAL_CAPABILITY)
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
        let mut offer = network_credentials_offer(
            CREDENTIAL_CAPABILITY.into(),
            i.implementation.clone(),
            i.artifact.clone(),
        );
        offer.resource_requirements = vec![resource_requirement(CREDENTIAL_RESOURCE_CLASS, 1)];
        registry
            .register(BaseProviderEntry {
                base_id: i.base.clone(),
                provider_instance_id: i.instance.clone(),
                provider_generation: i.provider_generation,
                implementation_id: i.base_implementation.clone(),
                mechanism_family: CREDENTIAL_BASE_KIND.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    i.resource.as_str(),
                    CREDENTIAL_RESOURCE_CLASS,
                    1,
                )],
            })
            .map_err(|_| E::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| E::Advertisement)?;
        advertisement
            .resources
            .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        Ok(())
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
    pub fn bind_selected(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PreparedCredentialContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<NetworkCredentialsHostCall<P>, CredentialCallRefusal> {
        Self::bind_selected_retained(&mut Some(self), table, handle, claim, contract, selected)
    }
    /// Validate native selection, exact lowering and table scope before transferring
    /// the actual owner. Every refusal leaves the caller's initialized slot intact.
    /// The opaque bearer is still authorized only when the Host Call is invoked.
    /// Issued table/handle arguments are consumed; refusal never invokes a provider.
    pub fn bind_selected_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PreparedCredentialContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<NetworkCredentialsHostCall<P>, CredentialCallRefusal> {
        let ready = slot
            .as_ref()
            .ok_or(CredentialCallRefusal::OwnerUnavailable)?;
        let gear = selected
            .fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selected.placement_id)
            .ok_or(CredentialCallRefusal::WrongRequest)?;
        ready.check_selected_native(&claim, gear, contract)?;
        let node =
            NetworkCredentialsHostCall::<P>::validate_selected(&table, &claim, contract, selected)?;
        BoundBaseOperation::check(&table, &handle, &claim)
            .map_err(CredentialCallRefusal::Capability)?;
        let ready = slot.take().expect("validated uniquely borrowed owner slot");
        Ok(NetworkCredentialsHostCall::bound(
            table,
            handle,
            claim,
            ready.source,
            node,
        ))
    }
    fn check_selected_native(
        &self,
        claim: &BaseOperationClaim,
        gear: &PlannedGear,
        contract: &PreparedCredentialContract,
    ) -> Result<(), CredentialCallRefusal> {
        self.check_selected_native_ref(claim, gear.into(), &contract.kind().limits)
    }
    pub(crate) fn check_selected_native_ref(
        &self,
        claim: &BaseOperationClaim,
        gear: conduit_core::PlannedGearRef<'_>,
        limits: &CapabilityLimits,
    ) -> Result<(), CredentialCallRefusal> {
        let base = gear.base.ok_or(CredentialCallRefusal::WrongRequest)?;
        let i = &self.identity;
        if !self.claim_matches(claim)
            || gear.artifact_id != i.artifact.as_str()
            || gear.implementation_id != i.implementation.as_str()
            || gear.execution_profile_id != CREDENTIAL_EXECUTION_PROFILE
            || gear.capability_id != CREDENTIAL_CAPABILITY
            || gear.limits != limits
            || base.base_id != i.base.as_str()
            || base.provider_instance_id != i.instance.as_str()
            || base.provider_generation != i.provider_generation
            || base.implementation_id != i.base_implementation.as_str()
            || base.mechanism_family != CREDENTIAL_BASE_KIND
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || !gear.authority.iter().any(|a| {
                a.contract_id == NETWORK_CREDENTIALS_AUTHORITY
                    && a.host_call_contract_id == NETWORK_CREDENTIALS_HOST_CALL
                    && a.subject_kind == NETWORK_CREDENTIALS_SUBJECT
            })
        {
            return Err(CredentialCallRefusal::WrongRequest);
        }
        Ok(())
    }

    /// Bind only after the same actual owner, Base, contract, resource and
    /// possession checks, using opaque immutable canonical direct tables.
    pub fn bind_direct_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PreparedCredentialContract,
        selected: conduit_plan_lowering::selected_operation::SelectedDirectOperationPlan<
            '_,
            3,
            2,
            1,
            3,
            2,
            3,
        >,
    ) -> Result<NetworkCredentialsHostCall<P>, CredentialCallRefusal> {
        let ready = slot
            .as_ref()
            .ok_or(CredentialCallRefusal::OwnerUnavailable)?;
        let (_, fragment) = selected.tables.preparation_scope();
        let gear = fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selected.placement_id)
            .ok_or(CredentialCallRefusal::WrongRequest)?;
        ready.check_selected_native(&claim, gear, contract)?;
        let node =
            NetworkCredentialsHostCall::<P>::validate_direct(&table, &claim, contract, selected)?;
        BoundBaseOperation::check(&table, &handle, &claim)
            .map_err(CredentialCallRefusal::Capability)?;
        let ready = slot.take().expect("validated unique initialized owner");
        Ok(NetworkCredentialsHostCall::bound(
            table,
            handle,
            claim,
            ready.source,
            node,
        ))
    }

    /// Consume the retained actual source using independently held native facts,
    /// not implementation identities supplied by an incoming assignment.
    pub fn bind_assigned(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PreparedCredentialContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<NetworkCredentialsHostCall<P>, CredentialCallRefusal> {
        if !self.claim_matches(&claim) {
            return Err(CredentialCallRefusal::WrongRequest);
        }
        let i = &self.identity;
        let native = AssignedNativeIdentity {
            capability: CREDENTIAL_CAPABILITY,
            execution_profile: CREDENTIAL_EXECUTION_PROFILE,
            artifact: i.artifact.as_str(),
            base: i.base.as_str(),
            base_implementation: i.base_implementation.as_str(),
            mechanism: CREDENTIAL_BASE_KIND,
            authority: NETWORK_CREDENTIALS_AUTHORITY,
            resource_class: CREDENTIAL_RESOURCE_CLASS,
        };
        // SAFETY: new retained the actual initialized source and native identity;
        // the common binder checks exact assignment/current possession.
        unsafe {
            NetworkCredentialsHostCall::bind_assigned(
                table,
                handle,
                claim,
                self.source,
                contract,
                native,
                selected,
            )
        }
    }
}
