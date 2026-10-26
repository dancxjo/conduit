//! Minimal immutable presenter identity after full ordinary Plan admission.
//! No configuration, authority, provider or topology is replaced by this view.
use super::*;
pub(super) trait Presenter {
    fn placement_id(&self) -> &PlacementId;
    fn capability_id(&self) -> &CapabilityId;
    fn implementation_id(&self) -> &ImplementationId;
    fn artifact_id(&self) -> &ArtifactId;
    fn host_id(&self) -> &HostId;
    fn boot_id(&self) -> &BootId;
    fn offer_generation(&self) -> OfferGeneration;
}
impl Presenter for PlannedGear {
    fn placement_id(&self) -> &PlacementId {
        &self.placement_id
    }
    fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }
    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }
    fn host_id(&self) -> &HostId {
        &self.host_id
    }
    fn boot_id(&self) -> &BootId {
        &self.boot_id
    }
    fn offer_generation(&self) -> OfferGeneration {
        self.offer_generation
    }
}
pub(crate) struct PreparedPresenterBasis {
    plan_id: PlanId,
    placement_id: PlacementId,
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
}
impl Presenter for PreparedPresenterBasis {
    fn placement_id(&self) -> &PlacementId {
        &self.placement_id
    }
    fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }
    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }
    fn host_id(&self) -> &HostId {
        &self.host_id
    }
    fn boot_id(&self) -> &BootId {
        &self.boot_id
    }
    fn offer_generation(&self) -> OfferGeneration {
        self.offer_generation
    }
}
impl PreparedPresenterBasis {
    /// Caller validated the complete ordinary Mask Plan before retaining this
    /// subset. Its complete configuration remains in the immutable wire cache.
    pub(crate) fn from_admitted_mask(planned: &crate::PlannedMaskPlot) -> Self {
        let placement = planned.show_placement();
        Self {
            plan_id: planned.plan.plan_id.clone(),
            placement_id: placement.placement_id.clone(),
            capability_id: placement.capability_id.clone(),
            implementation_id: placement.implementation_id.clone(),
            artifact_id: placement.artifact_id.clone(),
            host_id: placement.host_id.clone(),
            boot_id: placement.boot_id.clone(),
            offer_generation: placement.offer_generation,
        }
    }
    pub(crate) fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    pub(crate) fn prepare(
        &self,
        face: &Face,
        active: ActivePlayIdentity,
        front: String,
        target: String,
        sign: SignId,
    ) -> Result<Manifestation, ManifestationError> {
        face.validate()
            .map_err(|_| ManifestationError::InvalidFace)?;
        Manifestation::prepared_at_basis(face, &self.plan_id, active, self, front, target, sign)
    }
}
