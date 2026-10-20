//! Read-only evidence from an actual resident provider and its live reservation.
//! This interface observes a provider; it grants no resource or effect authority.
use super::{CostEvaluationContext, CostEvidenceRefusal, CostEvidenceSource};
use conduit_core::{
    verify_plan, BoundedResourceRef, Plan, PlannedGear, ResourceAdmissionOwner, ResourceBinding,
    ResourceHandleId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostResidenceRefusal {
    Evidence(CostEvidenceRefusal),
    InvalidPlan,
    PlacementChanged,
    OwnerChanged,
    MissingReservation,
    ResourceChanged,
    NotPublished,
    Lost,
    UnsupportedLifetime,
}

/// Implementations read their original immutable binding and publication state.
/// An implementation's provenance/trust remains a separate Host admission fact.
/// The concrete local provider requires exclusive mutation: its borrow fences
/// retirement during inspection. Other providers must supply their own
/// synchronization/lifetime law; this observation interface adds neither.
pub trait CostResourceResidence {
    fn cost_binding(&self) -> &ResourceBinding;
    fn cost_handle(&self) -> &ResourceHandleId;
    fn cost_published_reference(&self) -> Result<&BoundedResourceRef, CostResidenceRefusal>;
    fn cost_resident_bytes(&self) -> u64;
}

/// One current Plan placement, provider and resource-owner reservation. No
/// serialized snapshot can recreate this borrow or grant possession.
pub struct CostResidentResource<'a> {
    source: &'a CostEvidenceSource<'a>,
    plan: &'a Plan,
    placement: &'a PlannedGear,
    owner: &'a ResourceAdmissionOwner,
    provider: &'a dyn CostResourceResidence,
}
impl<'a> CostResidentResource<'a> {
    pub fn bind(
        source: &'a CostEvidenceSource<'a>,
        plan: &'a Plan,
        placement: &'a PlannedGear,
        owner: &'a ResourceAdmissionOwner,
        provider: &'a dyn CostResourceResidence,
    ) -> Result<Self, CostResidenceRefusal> {
        let evidence = Self {
            source,
            plan,
            placement,
            owner,
            provider,
        };
        evidence.validate_owner(owner)?;
        Ok(evidence)
    }

    fn validate_owner(&self, owner: &ResourceAdmissionOwner) -> Result<(), CostResidenceRefusal> {
        use CostResidenceRefusal as R;
        if !verify_plan(self.plan) {
            return Err(R::InvalidPlan);
        }
        let host = self.source.host();
        if owner.host() != host {
            return Err(R::OwnerChanged);
        }
        let p = self.placement;
        if !self
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .any(|g| g == p)
            || !matches_placement_source(p, self.source)
        {
            return Err(R::PlacementChanged);
        }
        let binding = self.provider.cost_binding();
        let resource = self.source.resource().ok_or(R::ResourceChanged)?;
        if !p.resources.contains(binding)
            || binding.pool_id != resource.pool_id
            || binding.class_id != resource.class_id
            || self.provider.cost_handle().as_str().is_empty()
            || self.provider.cost_handle().as_str().len() > super::MAXIMUM_COST_ID_BYTES
        {
            return Err(R::ResourceChanged);
        }
        let mut admissions = owner
            .admissions()
            .iter()
            .filter(|a| a.plan_id == self.plan.plan_id && a.placement_id == p.placement_id);
        let admission = admissions.next().ok_or(R::MissingReservation)?;
        if admissions.next().is_some()
            || admission.host_id != host.host_id
            || admission.boot_id != host.boot_id
            || admission.offer_generation != host.offer_generation
            || !admission.items.iter().any(|item| &item.binding == binding)
        {
            return Err(R::MissingReservation);
        }
        let content = binding.content.as_ref().ok_or(R::ResourceChanged)?;
        if content.owner_host != host.host_id || content.owner_boot != host.boot_id {
            return Err(R::ResourceChanged);
        }
        let reference = self.provider.cost_published_reference()?;
        if content.contract.retention != conduit_core::ResourceRetention::Play
            || reference.lifetime.expires_at.is_some()
        {
            return Err(R::UnsupportedLifetime);
        }
        content
            .contract
            .accepts_reference(reference)
            .map_err(|_| R::ResourceChanged)?;
        if reference.access_class != binding.class_id
            || self.provider.cost_resident_bytes() != reference.extent.bytes
        {
            return Err(R::ResourceChanged);
        }
        Ok(())
    }

    /// Fresh inventory, calibration, clock and original reservation are checked
    /// independently. A current provider alone cannot refresh an expired sample.
    pub fn validate_at(
        &self,
        context: &CostEvaluationContext<'_>,
        owner: &ResourceAdmissionOwner,
    ) -> Result<(), CostResidenceRefusal> {
        if !context.calibrations.contains(&self.source.calibration()) {
            return Err(CostResidenceRefusal::Evidence(CostEvidenceRefusal::Scope(
                super::CostScopeChange::Calibration,
            )));
        }
        self.source
            .validate_at(context.host, self.source.calibration(), context.now)
            .map_err(CostResidenceRefusal::Evidence)?;
        self.validate_owner(owner)
    }
    pub fn source(&self) -> &'a CostEvidenceSource<'a> {
        self.source
    }
    pub fn plan(&self) -> &'a Plan {
        self.plan
    }
    pub fn placement(&self) -> &'a PlannedGear {
        self.placement
    }
    pub fn provider(&self) -> &'a dyn CostResourceResidence {
        self.provider
    }
    pub fn owner(&self) -> &'a ResourceAdmissionOwner {
        self.owner
    }
}

/// Cost provenance must name the exact selected Back, not just a reusable
/// capability label whose artifact or profile might have changed in place.
pub(super) fn matches_placement_source(p: &PlannedGear, source: &CostEvidenceSource<'_>) -> bool {
    let host = source.host();
    let offer = source.offer();
    let mut owners = host
        .bases
        .iter()
        .filter(|b| b.capability_ids.contains(&offer.capability_id));
    let owner = owners.next();
    let base_matches = owners.next().is_none()
        && match (&p.base, owner) {
            (None, None) => true,
            (Some(planned), Some(current)) => {
                current.lifecycle == conduit_core::BaseLifecycle::Ready
                    && current.provider_generation > 0
                    && planned == &current.binding()
            }
            _ => false,
        };
    base_matches
        && p.host_id == host.host_id
        && p.boot_id == host.boot_id
        && p.offer_generation == host.offer_generation
        && p.capability_id == offer.capability_id
        && p.kind_id == offer.kind_id
        && p.kind_contract_revision == offer.kind_contract_revision
        && p.execution_profile_id == offer.implementation.execution_profile_id
        && p.implementation_id == offer.implementation.implementation_id
        && p.artifact_id == offer.implementation.artifact_id
        && p.inputs == offer.inputs
        && p.outputs == offer.outputs
        && p.semantic_contract == offer.semantic_contract
        && p.host_calls == offer.host_calls
        && p.limits == offer.limits
        && p.realization_properties == offer.realization_properties
}
