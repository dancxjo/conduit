//! Project the actually retained renderer through the ordinary Base registry.
use super::*;
impl<P: OpenDrain> ReadyRenderer<P> {
    /// Only this retained provider may project readiness in its exact Host/Boot.
    /// The ordinary registry preserves uniqueness and finite offer capacity.
    /// Projection performs no effect and grants no authority.
    pub fn append_to_advertisement(
        &self,
        host: &mut c::HostAdvertisement,
    ) -> Result<(), &'static str> {
        let id = &self.identity;
        if host.host_id != id.host_id || host.boot_id != id.boot_id || host.offer_generation.0 == 0
        {
            return Err("renderer-offer-scope");
        }
        if host.bases.iter().any(|base| {
            base.base_id == id.base_id || base.provider_instance_id == id.provider_instance_id
        }) || host
            .capabilities
            .iter()
            .any(|offer| offer.capability_id.as_str() == CAPABILITY)
            || host
                .resources
                .iter()
                .any(|resource| resource.pool_id == id.resource_pool_id)
        {
            return Err("renderer-offer-duplicate");
        }
        let mut registry = c::BaseRegistry::new(c::BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(
                host.capabilities.len().saturating_add(1),
            )
            .map_err(|_| "renderer-offer-capacity")?,
            maximum_advertised_resources: u16::try_from(host.resources.len().saturating_add(1))
                .map_err(|_| "renderer-offer-capacity")?,
        })
        .map_err(|_| "renderer-offer-registry")?;
        registry
            .register(self.ready_entry())
            .map_err(|_| "renderer-offer-registry")?;
        registry
            .project_ready_into(host)
            .map_err(|_| "renderer-offer-projection")
    }
    fn ready_entry(&self) -> c::BaseProviderEntry {
        let id = &self.identity;
        let offer = c::BackOfferBuilder::new(
            f::renderer_kind(),
            c::Back {
                capability_id: CAPABILITY.into(),
                execution_profile_id: PROFILE.into(),
                implementation_id: id.implementation_id.clone(),
                artifact_id: id.artifact_id.clone(),
                host_calls: vec![c::HostCallRequirement {
                    contract_id: CALL.into(),
                    target_kind: Some(f::RENDERER_KIND.into()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: self.bounds.face_bytes,
                    maximum_output_bytes: self.bounds.show_bytes,
                }],
                resource_requirements: vec![c::resource_requirement(ATTACHMENT, 1)],
                authority_requirements: vec![c::AuthorityRequirement {
                    contract_id: AUTHORITY.into(),
                    host_call_contract_id: CALL.into(),
                    subject_kind: f::RENDERER_KIND.into(),
                }],
            },
        )
        .narrow_capacity(c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: self.bounds.face_bytes.max(self.bounds.show_bytes),
        })
        .expect("bounded renderer capacity narrows portable semantics")
        .build();
        c::BaseProviderEntry {
            base_id: id.base_id.clone(),
            provider_instance_id: id.provider_instance_id.clone(),
            provider_generation: id.provider_generation,
            implementation_id: id.base_implementation_id.clone(),
            mechanism_family: ATTACHMENT.into(),
            enforcement_class: c::BaseEnforcementClass::Cooperative,
            lifecycle: c::BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![c::resource_offer(
                id.resource_pool_id.as_str(),
                ATTACHMENT,
                1,
            )],
        }
    }
}
