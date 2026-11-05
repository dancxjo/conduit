//! Read-only staging for the same boot-scoped reservation owner.
use super::*;
impl ResourceAdmissionOwner {
    /// The same read-only law serves one request and an atomic batch. Staged
    /// admissions participate in every cumulative and exclusivity check.
    pub(super) fn prepare_admission(
        &self,
        staged: &[ResourceAdmission],
        request: ResourceAdmissionRequest,
        observations: &[ResourceObservation],
    ) -> Result<ResourceAdmission, ResourceAdmissionRefusal> {
        if request.items.is_empty() {
            return Err(ResourceAdmissionRefusal::Empty);
        }
        if request.items.len() > MAXIMUM_BINDINGS_PER_ADMISSION
            || self.admissions.len() + staged.len() >= MAXIMUM_RESOURCE_ADMISSIONS
        {
            return Err(ResourceAdmissionRefusal::CapacityExceeded);
        }
        if request.host_id != self.host.host_id {
            return Err(ResourceAdmissionRefusal::ForeignHost);
        }
        if request.boot_id != self.host.boot_id
            || request.offer_generation != self.host.offer_generation
        {
            return Err(ResourceAdmissionRefusal::StaleOffer);
        }
        if self.admissions.iter().chain(staged).any(|admission| {
            admission.plan_id == request.plan_id && admission.placement_id == request.placement_id
        }) {
            return Err(ResourceAdmissionRefusal::DuplicatePlanPlacement);
        }

        let mut signs = Vec::new();
        signs
            .try_reserve_exact(request.items.len())
            .map_err(|_| ResourceAdmissionRefusal::Allocation)?;
        for (index, item) in request.items.iter().enumerate() {
            let pool = self
                .host
                .resources
                .iter()
                .find(|pool| {
                    pool.pool_id == item.binding.pool_id && pool.class_id == item.binding.class_id
                })
                .ok_or(ResourceAdmissionRefusal::InvalidBinding)?;
            if !resource_binding_satisfies(&item.binding, &item.requirement, pool)
                || crate::resource_content::validate_content_residence(
                    &item.requirement,
                    pool,
                    &self.host.host_id,
                    &self.host.boot_id,
                )
                .is_err()
            {
                return Err(ResourceAdmissionRefusal::InvalidBinding);
            }
            if item.binding.content.as_ref().is_some_and(|content| {
                content.contract.access == crate::ResourceAccessMode::WriteCandidatePublish
            }) && self
                .admissions
                .iter()
                .chain(staged)
                .flat_map(|admission| &admission.items)
                .any(|existing| {
                    existing.binding.pool_id == item.binding.pool_id
                        && existing.binding.content.as_ref().is_some_and(|content| {
                            content.contract.access
                                == crate::ResourceAccessMode::WriteCandidatePublish
                        })
                })
            {
                return Err(ResourceAdmissionRefusal::CompetingResourceWriter);
            }
            let observation = observations
                .iter()
                .find(|observation| {
                    observation.pool_id == pool.pool_id && observation.class_id == pool.class_id
                })
                .ok_or(ResourceAdmissionRefusal::MissingObservation)?;
            if observation.host_id != self.host.host_id
                || observation.boot_id != self.host.boot_id
                || observation.offer_generation != self.host.offer_generation
            {
                return Err(ResourceAdmissionRefusal::StaleObservation);
            }
            if observation.health != ResourceHealth::Ready {
                return Err(ResourceAdmissionRefusal::Unavailable);
            }
            let already = self
                .admissions
                .iter()
                .chain(staged)
                .flat_map(|admission| &admission.items)
                .filter(|reserved| {
                    reserved.binding.pool_id == pool.pool_id
                        && reserved.binding.class_id == pool.class_id
                })
                .try_fold(0u32, |total, reserved| {
                    total.checked_add(reserved.binding.units)
                })
                .ok_or(ResourceAdmissionRefusal::Overcommitted)?;
            let within_request = request.items[..index]
                .iter()
                .filter(|prior| {
                    prior.binding.pool_id == pool.pool_id && prior.binding.class_id == pool.class_id
                })
                .try_fold(0u32, |total, prior| total.checked_add(prior.binding.units))
                .ok_or(ResourceAdmissionRefusal::Overcommitted)?;
            let total = already
                .checked_add(within_request)
                .and_then(|value| value.checked_add(item.binding.units))
                .ok_or(ResourceAdmissionRefusal::Overcommitted)?;
            if total > observation.unreserved_units || total > pool.capacity_units {
                return Err(ResourceAdmissionRefusal::Overcommitted);
            }
            let mut sign = alloc::string::String::new();
            sign.try_reserve_exact(observation.sign_id.as_str().len())
                .map_err(|_| ResourceAdmissionRefusal::Allocation)?;
            sign.push_str(observation.sign_id.as_str());
            signs.push(crate::SignId::new(sign));
        }
        signs.sort_unstable();
        signs.dedup();
        Ok(ResourceAdmission {
            plan_id: request.plan_id,
            placement_id: request.placement_id,
            host_id: request.host_id,
            boot_id: request.boot_id,
            offer_generation: request.offer_generation,
            items: request.items,
            observation_sign_ids: signs,
        })
    }
}
