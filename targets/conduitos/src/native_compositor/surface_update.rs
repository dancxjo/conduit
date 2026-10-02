//! Exact retained raster staging, separately from successful scanout.
use super::*;

impl NativeCompositor {
    pub fn update_surface(
        &mut self,
        presentation: &Presentation,
        manifestation: &Manifestation,
        plan: &Plan,
        surface_id: &str,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        self.validate_manifestation(
            presentation,
            manifestation,
            plan,
            surface_id,
            display_base_id,
        )?;
        self.raster_surface(
            presentation,
            manifestation,
            surface_id,
            display_base_id,
            scene,
            true,
            false,
        )
    }
    /// Stage a still-pending ordinary Mask renderer, without granting input ownership.
    /// Only successfully composed surface pixels may enable input and mint scanout evidence.
    pub fn update_pending_mask_surface(
        &mut self,
        presentation: &Presentation,
        show: &conduit_presentation::MaskShow,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        show.validate(presentation)
            .map_err(|_| NativeCompositorError::ManifestationInvalid)?;
        if show.show.lifecycle != ManifestationLifecycle::Prepared {
            return Err(NativeCompositorError::StaleIdentity);
        }
        self.validate_surface_binding(&show.show, &show.show.target_subject, display_base_id)?;
        self.raster_surface(
            presentation,
            &show.show,
            &show.show.target_subject,
            display_base_id,
            scene,
            false,
            false,
        )
    }
    /// Repaint Mask-local focus, pagination, or uncommitted input under the
    /// same acknowledged Show. The semantic Face revision must be identical;
    /// this operation cannot mint a new Show or advance application truth.
    pub fn repaint_mask_surface(
        &mut self,
        presentation: &Presentation,
        show: &conduit_presentation::MaskShow,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        show.validate(presentation)
            .map_err(|_| NativeCompositorError::ManifestationInvalid)?;
        if show.show.lifecycle != ManifestationLifecycle::Available {
            return Err(NativeCompositorError::StaleIdentity);
        }
        self.validate_surface_binding(&show.show, &show.show.target_subject, display_base_id)?;
        self.raster_surface(
            presentation,
            &show.show,
            &show.show.target_subject,
            display_base_id,
            scene,
            false,
            true,
        )
    }
    fn raster_surface(
        &mut self,
        presentation: &Presentation,
        manifestation: &Manifestation,
        surface_id: &str,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
        input_ready: bool,
        same_revision: bool,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        let surface = self
            .surfaces
            .iter_mut()
            .find(|surface| surface.surface_id == surface_id)
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)?;
        if let Some(binding) = &surface.binding {
            if binding.plan_id != manifestation.plan_id
                || binding.placement_id != manifestation.placement_id
                || binding.front_subject != manifestation.front_subject
            {
                return Err(NativeCompositorError::SurfaceAlreadyBound);
            }
            if (same_revision
                && (presentation.revision != binding.last_revision
                    || manifestation.manifestation_id != binding.manifestation_id))
                || (!same_revision && presentation.revision <= binding.last_revision)
            {
                return Err(NativeCompositorError::StaleSurfaceRevision);
            }
        }
        // A failed raster update must not leave the old revision routable.
        surface.receipt = None;
        surface.input_ready = false;
        self.scanout_pixels = [0; MAX_COMPOSITOR_SURFACES];
        surface.buffer.clear();
        #[cfg(feature = "native-compositor")]
        let display =
            crate::display::typography::render_scene(&mut surface.buffer, scene, |_, command| {
                command.text_role().into()
            })?;
        #[cfg(not(feature = "native-compositor"))]
        let display = render_scene(&mut surface.buffer, scene)?;
        surface.binding = Some(SurfaceBinding {
            manifestation_id: manifestation.manifestation_id.clone(),
            plan_id: manifestation.plan_id.clone(),
            placement_id: manifestation.placement_id.clone(),
            front_subject: manifestation.front_subject.clone(),
            last_revision: presentation.revision,
        });
        surface.input_ready = input_ready;
        surface.receipt = Some(CompositionReceipt {
            presentation_id: presentation.identity.clone(),
            manifestation_id: manifestation.manifestation_id.clone(),
            plan_id: manifestation.plan_id.clone(),
            active_play_id: manifestation.active_play_id.clone(),
            play_sequence: manifestation.play_sequence,
            placement_id: manifestation.placement_id.clone(),
            host_id: manifestation.host_id.clone(),
            boot_id: manifestation.boot_id.clone(),
            offer_generation: manifestation.offer_generation,
            presenter_implementation_id: manifestation.presenter_implementation_id.clone(),
            presenter_capability_id: manifestation.presenter_capability_id.clone(),
            presenter_artifact_id: manifestation.presenter_artifact_id.clone(),
            front_subject: manifestation.front_subject.clone(),
            display_base_id: display_base_id.clone(),
            surface_id: surface_id.into(),
            display,
        });
        if surface.visible {
            self.damage.add_layout(surface.bounds)?;
        }
        surface
            .receipt
            .as_ref()
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)
    }
    fn validate_manifestation(
        &self,
        presentation: &Presentation,
        manifestation: &Manifestation,
        plan: &Plan,
        surface_id: &str,
        display_base_id: &HostBaseId,
    ) -> Result<(), NativeCompositorError> {
        self.validate_surface_binding(manifestation, surface_id, display_base_id)?;
        if manifestation.lifecycle != ManifestationLifecycle::Available {
            return Err(NativeCompositorError::StaleIdentity);
        }
        manifestation
            .validate_against(presentation, plan)
            .map_err(map_manifestation_error)?;
        Ok(())
    }
    fn validate_surface_binding(
        &self,
        manifestation: &Manifestation,
        surface_id: &str,
        display_base_id: &HostBaseId,
    ) -> Result<(), NativeCompositorError> {
        self.validate_surface_id(surface_id)?;
        if !self
            .admission
            .placement_ids
            .contains(&manifestation.placement_id)
        {
            return Err(NativeCompositorError::UnadmittedPlacement);
        }
        if manifestation.host_id != self.admission.host_id
            || manifestation.boot_id != self.admission.boot_id
            || manifestation.offer_generation != self.admission.offer_generation
            || manifestation.presenter_implementation_id
                != self.admission.presenter_implementation_id
            || display_base_id != &self.admission.display_base_id
            || manifestation.target_subject != surface_id
        {
            return Err(NativeCompositorError::StaleIdentity);
        }
        Ok(())
    }
}
