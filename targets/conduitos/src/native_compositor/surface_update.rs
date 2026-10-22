//! Exact retained raster staging, separately from successful scanout.
use super::*;
#[cfg(feature = "native-compositor")]
use crate::display::PixelTarget;

#[derive(Clone, Copy, Eq, PartialEq)]
enum RasterMode {
    Available,
    Pending,
    Repaint,
}

impl NativeCompositor {
    pub fn update_surface(
        &mut self,
        face: &Face,
        manifestation: &Manifestation,
        plan: &Plan,
        surface_id: &str,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        self.validate_manifestation(face, manifestation, plan, surface_id, display_base_id)?;
        self.raster_surface(
            face,
            manifestation,
            surface_id,
            display_base_id,
            scene,
            RasterMode::Available,
        )
    }
    /// Stage a still-pending ordinary Mask renderer, without granting input ownership.
    /// Only successfully composed surface pixels may enable input and mint scanout evidence.
    pub fn update_pending_mask_surface(
        &mut self,
        face: &Face,
        show: &conduit_face::MaskShow,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        show.validate(face)
            .map_err(|_| NativeCompositorError::ManifestationInvalid)?;
        if show.show.lifecycle != ManifestationLifecycle::Prepared {
            return Err(NativeCompositorError::StaleIdentity);
        }
        self.validate_surface_binding(&show.show, &show.show.target_subject, display_base_id)?;
        self.raster_surface(
            face,
            &show.show,
            &show.show.target_subject,
            display_base_id,
            scene,
            RasterMode::Pending,
        )
    }
    /// Stage an admitted expression on this exact pending Mask occurrence.
    /// A different Plan or unchanged Face revision requires old-surface retirement.
    pub fn update_pending_configured_mask_surface(
        &mut self,
        face: &Face,
        show: &conduit_face::MaskShow,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
        expression: &conduit_face::AdmittedVisualExpression,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        if !(100..=200).contains(&expression.expression.text_scale_percent) {
            return Err(NativeCompositorError::Display(DisplayError::InvalidExtent));
        }
        show.validate(face)
            .map_err(|_| NativeCompositorError::ManifestationInvalid)?;
        if show.show.lifecycle != ManifestationLifecycle::Prepared {
            return Err(NativeCompositorError::StaleIdentity);
        }
        self.validate_surface_binding(&show.show, &show.show.target_subject, display_base_id)?;
        let surface = self
            .surfaces
            .iter_mut()
            .find(|surface| surface.surface_id == show.show.target_subject)
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)?;
        if let Some(binding) = &surface.binding {
            if binding.plan_id != show.show.plan_id
                || binding.placement_id != show.show.placement_id
                || binding.front_subject != show.show.front_subject
            {
                return Err(NativeCompositorError::SurfaceAlreadyBound);
            }
            if face.revision <= binding.last_revision {
                return Err(NativeCompositorError::StaleSurfaceRevision);
            }
        }
        surface.visual_expression = Some(expression.clone());
        self.raster_surface(
            face,
            &show.show,
            &show.show.target_subject,
            display_base_id,
            scene,
            RasterMode::Pending,
        )
    }

    /// Repaint Mask-local focus, pagination, or uncommitted input under the
    /// same acknowledged Show. The semantic Face revision must be identical;
    /// this operation cannot mint a new Show or advance application truth.
    pub fn repaint_mask_surface(
        &mut self,
        face: &Face,
        show: &conduit_face::MaskShow,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        show.validate(face)
            .map_err(|_| NativeCompositorError::ManifestationInvalid)?;
        if show.show.lifecycle != ManifestationLifecycle::Available {
            return Err(NativeCompositorError::StaleIdentity);
        }
        self.validate_surface_binding(&show.show, &show.show.target_subject, display_base_id)?;
        self.raster_surface(
            face,
            &show.show,
            &show.show.target_subject,
            display_base_id,
            scene,
            RasterMode::Repaint,
        )
    }
    fn raster_surface(
        &mut self,
        face: &Face,
        manifestation: &Manifestation,
        surface_id: &str,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
        mode: RasterMode,
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
            if (mode == RasterMode::Repaint
                && (face.revision != binding.last_revision
                    || manifestation.manifestation_id != binding.manifestation_id))
                || (mode != RasterMode::Repaint && face.revision <= binding.last_revision)
            {
                return Err(NativeCompositorError::StaleSurfaceRevision);
            }
        }
        // A failed raster update must not leave the old revision routable.
        let previous = surface.raster_scene.take().filter(|_| {
            surface
                .receipt
                .as_ref()
                .is_some_and(|receipt| receipt.visual_expression == surface.visual_expression)
        });
        let cacheable = super::raster_reuse::canonical(scene);
        let previous = previous.as_ref().filter(|_| cacheable);
        surface.receipt = None;
        surface.input_ready = false;
        self.scanout_pixels = [0; MAX_COMPOSITOR_SURFACES];
        #[cfg(feature = "native-compositor")]
        let display = {
            let format = surface.buffer.format();
            let expression = surface.visual_expression.as_ref();
            super::raster_reuse::raster(&mut surface.buffer, scene, previous, |buffer, replay| {
                crate::display::typography::render_scene_selected(
                    buffer,
                    replay,
                    |_, command| command.text_role().into(),
                    expression.map_or(100, |value| value.expression.text_scale_percent),
                    crate::display::typography::Typography::new(
                        expression.map_or(conduit_face::VisualTypography::Editorial, |value| {
                            value.expression.typography
                        }),
                    ),
                    |role| visual_paint(format, role, expression),
                )
            })?
        };
        #[cfg(not(feature = "native-compositor"))]
        let display =
            super::raster_reuse::raster(&mut surface.buffer, scene, previous, render_scene)?;
        surface.raster_scene = cacheable.then_some(*scene);
        surface.binding = Some(SurfaceBinding {
            manifestation_id: manifestation.manifestation_id.clone(),
            plan_id: manifestation.plan_id.clone(),
            placement_id: manifestation.placement_id.clone(),
            front_subject: manifestation.front_subject.clone(),
            last_revision: face.revision,
        });
        surface.input_ready = mode == RasterMode::Available;
        surface.receipt = Some(CompositionReceipt {
            face_id: face.identity.clone(),
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
            visual_expression: surface.visual_expression.clone(),
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
        face: &Face,
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
            .validate_against(face, plan)
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

#[cfg(feature = "native-compositor")]
use super::paint::visual_paint;

#[cfg(all(test, feature = "native-compositor"))]
#[path = "surface_update/paint_tests.rs"]
mod paint_tests;
