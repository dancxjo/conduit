//! Compact immutable Show preparation context; never execution authority.
use super::*;

/// Renderer identity borrowed from independently selected Source and the actual
/// retained native provider. A received assignment cannot select these facts.
pub struct AssignedRendererSource<'a> {
    pub gear: &'a conduit_core::GearId,
    pub host: &'a HostId,
    pub boot: &'a BootId,
    pub capability: &'a CapabilityId,
    pub implementation: &'a ImplementationId,
    pub artifact: &'a ArtifactId,
    pub generation: OfferGeneration,
}
pub struct PreparedRendererContext {
    plan: PlanId,
    placement: PlacementId,
    host: HostId,
    boot: BootId,
    capability: CapabilityId,
    implementation: ImplementationId,
    artifact: ArtifactId,
    generation: OfferGeneration,
}
impl PreparedRendererContext {
    pub fn prepare(plan: &Plan, placement_id: &PlacementId) -> Result<Self, ManifestationError> {
        let placement = renderer_placement(plan, placement_id)?;
        Ok(Self {
            plan: plan.plan_id.clone(),
            placement: placement.placement_id.clone(),
            host: placement.host_id.clone(),
            boot: placement.boot_id.clone(),
            capability: placement.capability_id.clone(),
            implementation: placement.implementation_id.clone(),
            artifact: placement.artifact_id.clone(),
            generation: placement.offer_generation,
        })
    }
    /// Construct the same immutable context after compact assigned preparation.
    /// # Safety
    /// Root must first validate the exact assigned Mask tables, including this
    /// Source-selected renderer node/configuration, and retain the original
    /// provider and capability ticket. `source` must borrow that actual provider
    /// and independently compiled Gear identity, never incoming descriptors.
    /// This allocates preparation metadata, not a grant or an Available Show.
    #[cfg(feature = "assigned-renderer")]
    pub unsafe fn from_assigned(
        approved: &conduit_body::DelegatedBodyFragment<'_>,
        source: AssignedRendererSource<'_>,
    ) -> Result<Self, ManifestationError> {
        let metadata = approved.metadata();
        if metadata.host != source.host.as_str()
            || metadata.boot != source.boot.as_str()
            || metadata.offer_generation != source.generation.0
            || source.generation.0 == 0
        {
            return Err(ManifestationError::StaleIdentity);
        }
        // This is the ordinary planner's deterministic placement identity. It
        // retains no invented Plan or discarded port/configuration metadata.
        let mut hash = Sha256::new();
        use core::fmt::Write;
        let text = alloc::format!(
            "placement:{}:{}:{}:{}",
            metadata.checked_plot,
            source.gear.as_str(),
            source.host.as_str(),
            source.capability.as_str()
        );
        hash.update(text.as_bytes());
        let mut id = String::with_capacity(64);
        for byte in hash.finalize() {
            let _ = write!(id, "{byte:02x}");
        }
        Ok(Self {
            plan: metadata.plot_plan.into(),
            placement: id.into(),
            host: source.host.clone(),
            boot: source.boot.clone(),
            capability: source.capability.clone(),
            implementation: source.implementation.clone(),
            artifact: source.artifact.clone(),
            generation: source.generation,
        })
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        [
            self.plan.0.capacity(),
            self.placement.0.capacity(),
            self.host.0.capacity(),
            self.boot.0.capacity(),
            self.capability.0.capacity(),
            self.implementation.0.capacity(),
            self.artifact.0.capacity(),
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
    }
    pub fn prepared_show(
        &self,
        face: &Face,
        active: ActivePlayIdentity,
        front: String,
        target: String,
        sign: SignId,
    ) -> Result<Manifestation, ManifestationError> {
        face.validate()
            .map_err(|_| ManifestationError::InvalidFace)?;
        if active != bind_active_play(&self.plan, &self.host, &self.boot, active.play_sequence) {
            return Err(ManifestationError::StaleIdentity);
        }
        validate_target(&front)?;
        validate_front_subject(face, &front)?;
        validate_target(&target)?;
        let mut hash = Sha256::new();
        for value in [
            "conduit.face/show@1",
            face.identity.as_str(),
            self.plan.as_str(),
            active.active_play_id.as_str(),
            self.placement.as_str(),
            self.host.as_str(),
            self.boot.as_str(),
            self.capability.as_str(),
            self.implementation.as_str(),
            self.artifact.as_str(),
            &front,
            &target,
        ] {
            hash.update((value.len() as u32).to_le_bytes());
            hash.update(value.as_bytes());
        }
        hash.update(self.generation.0.to_le_bytes());
        let mut identity = String::with_capacity(64);
        use core::fmt::Write;
        for byte in hash.finalize() {
            let _ = write!(identity, "{byte:02x}");
        }
        let mut show = Manifestation {
            manifestation_id: ManifestationId(identity),
            body_id: face.basis.body_id.clone(),
            wake_id: face.basis.wake_id.clone(),
            face_id: face.identity.clone(),
            face_revision: face.revision,
            plan_id: self.plan.clone(),
            active_play_id: active.active_play_id,
            play_sequence: active.play_sequence,
            placement_id: self.placement.clone(),
            front_subject: front,
            presenter_capability_id: self.capability.clone(),
            presenter_implementation_id: self.implementation.clone(),
            presenter_artifact_id: self.artifact.clone(),
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            offer_generation: self.generation,
            target_subject: target,
            lifecycle: ManifestationLifecycle::Prepared,
            failure: None,
            signs: alloc::vec::Vec::new(),
        };
        show.push_sign(sign);
        Ok(show)
    }
}
