//! Projection of this retained provider through the ordinary Base registry.
use super::*;
use alloc::vec;
impl<P: OpenDrain> ReadyWriteBase<P> {
    /// Project only the actual retained provider into the matching Host/Boot.
    /// Preparation may allocate; this method performs no electrical work.
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &WriteContract,
    ) -> Result<(), InstallationRefusal> {
        use InstallationRefusal as R;
        let identity = &self.identity;
        if advertisement.host_id != identity.host_id || advertisement.boot_id != identity.boot_id {
            return Err(R::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == identity.base_id
                || base.provider_instance_id == identity.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|c| c.capability_id.as_str() == WRITE_CAPABILITY)
            || advertisement
                .resources
                .iter()
                .any(|r| r.pool_id == identity.resource_pool_id)
        {
            return Err(R::Advertisement);
        }
        let kind = contract.kind();
        let offer = BackOfferBuilder::new(
            kind.clone(),
            Back {
                capability_id: WRITE_CAPABILITY.into(),
                execution_profile_id: WRITE_EXECUTION_PROFILE.into(),
                implementation_id: identity.implementation_id.clone(),
                artifact_id: identity.artifact_id.clone(),
                host_calls: vec![HostCallRequirement {
                    contract_id: WRITE_CALL.into(),
                    target_kind: Some(kind.kind_id.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MAXIMUM_BYTES,
                    maximum_output_bytes: 0,
                }],
                resource_requirements: vec![resource_requirement(WRITE_ATTACHMENT_CLASS, 1)],
                authority_requirements: vec![AuthorityRequirement {
                    contract_id: WRITE_AUTHORITY.into(),
                    host_call_contract_id: WRITE_CALL.into(),
                    subject_kind: kind.kind_id.clone(),
                }],
            },
        )
        .build();
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(
                advertisement.capabilities.len().saturating_add(1),
            )
            .map_err(|_| R::Advertisement)?,
            maximum_advertised_resources: u16::try_from(
                advertisement.resources.len().saturating_add(1),
            )
            .map_err(|_| R::Advertisement)?,
        })
        .map_err(|_| R::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: identity.base_id.clone(),
                provider_instance_id: identity.provider_instance_id.clone(),
                provider_generation: identity.provider_generation,
                implementation_id: identity.base_implementation_id.clone(),
                mechanism_family: WRITE_ATTACHMENT_CLASS.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    identity.resource_pool_id.as_str(),
                    WRITE_ATTACHMENT_CLASS,
                    1,
                )],
            })
            .map_err(|_| R::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| R::Advertisement)
    }
}
