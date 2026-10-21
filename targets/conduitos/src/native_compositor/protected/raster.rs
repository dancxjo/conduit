use super::*;
use conduit_core::{HostBaseId, Plan};
use conduit_face::{Face, Manifestation, ManifestationLifecycle, MaskShow};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
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
        id: &str,
        display: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        self.validate_binding(manifestation, id, display)?;
        if manifestation.lifecycle != ManifestationLifecycle::Available {
            return Err(NativeCompositorError::StaleIdentity);
        }
        manifestation
            .validate_against(face, plan)
            .map_err(manifestation_error)?;
        self.raster(face, manifestation, id, scene, Mode::Available)
    }
    pub fn update_pending_mask_surface(
        &mut self,
        face: &Face,
        show: &MaskShow,
        display: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        self.pending(face, show, display)?;
        self.raster(
            face,
            &show.show,
            &show.show.target_subject,
            scene,
            Mode::Pending,
        )
    }
    pub fn update_pending_configured_mask_surface(
        &mut self,
        face: &Face,
        show: &MaskShow,
        display: &HostBaseId,
        scene: &GraphicsScene,
        expression: &conduit_face::AdmittedVisualExpression,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        if !(100..=200).contains(&expression.expression.text_scale_percent) {
            return Err(crate::display::DisplayError::InvalidExtent.into());
        }
        self.pending(face, show, display)?;
        let i = self.index(&show.show.target_subject)?;
        self.check_revision(i, face, &show.show, Mode::Pending)?;
        self.surfaces[i].expression = Some(expression.clone());
        self.raster(
            face,
            &show.show,
            &show.show.target_subject,
            scene,
            Mode::Pending,
        )
    }
    pub fn repaint_mask_surface(
        &mut self,
        face: &Face,
        show: &MaskShow,
        display: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        show.validate(face)
            .map_err(|_| NativeCompositorError::ManifestationInvalid)?;
        if show.show.lifecycle != ManifestationLifecycle::Available {
            return Err(NativeCompositorError::StaleIdentity);
        }
        self.validate_binding(&show.show, &show.show.target_subject, display)?;
        self.raster(
            face,
            &show.show,
            &show.show.target_subject,
            scene,
            Mode::Repaint,
        )
    }
    fn pending(
        &self,
        face: &Face,
        show: &MaskShow,
        display: &HostBaseId,
    ) -> Result<(), NativeCompositorError> {
        show.validate(face)
            .map_err(|_| NativeCompositorError::ManifestationInvalid)?;
        if show.show.lifecycle != ManifestationLifecycle::Prepared {
            return Err(NativeCompositorError::StaleIdentity);
        }
        self.validate_binding(&show.show, &show.show.target_subject, display)
    }
    fn validate_binding(
        &self,
        m: &Manifestation,
        id: &str,
        display: &HostBaseId,
    ) -> Result<(), NativeCompositorError> {
        self.validate_id(id)?;
        if !self.admission.placement_ids.contains(&m.placement_id) {
            return Err(NativeCompositorError::UnadmittedPlacement);
        }
        if m.host_id != self.admission.host_id
            || m.boot_id != self.admission.boot_id
            || m.offer_generation != self.admission.offer_generation
            || m.presenter_implementation_id != self.admission.presenter_implementation_id
            || display != &self.admission.display_base_id
            || m.target_subject != id
        {
            return Err(NativeCompositorError::StaleIdentity);
        }
        Ok(())
    }
    fn check_revision(
        &self,
        i: usize,
        face: &Face,
        m: &Manifestation,
        mode: Mode,
    ) -> Result<(), NativeCompositorError> {
        if let Some(binding) = &self.surfaces[i].binding {
            if binding.plan != m.plan_id
                || binding.placement != m.placement_id
                || binding.front != m.front_subject
            {
                return Err(NativeCompositorError::SurfaceAlreadyBound);
            }
            if (mode == Mode::Repaint
                && (face.revision != binding.face_revision
                    || m.manifestation_id != binding.manifestation))
                || (mode != Mode::Repaint && face.revision <= binding.face_revision)
            {
                return Err(NativeCompositorError::StaleSurfaceRevision);
            }
        }
        Ok(())
    }
    fn raster(
        &mut self,
        face: &Face,
        m: &Manifestation,
        id: &str,
        scene: &GraphicsScene,
        mode: Mode,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        let i = self.index(id)?;
        self.check_revision(i, face, m, mode)?;
        let sequence = self.surfaces[i]
            .upload_sequence
            .checked_add(1)
            .ok_or(NativeCompositorError::ServiceLost)?;
        let handle = self.surfaces[i].handle;
        let bounds = self.surfaces[i].bounds;
        let expression = self.surfaces[i].expression.clone();
        self.invalidate();
        self.surfaces[i].receipt = None;
        self.surfaces[i].recipe = None;
        if let Err(error) = self.unit(Request::BeginUpload {
            surface: handle,
            revision: sequence,
            rect: rect(bounds),
        }) {
            self.fail();
            return Err(error);
        }
        let session = self.scope()?;
        let owner = self.owner()?.clone();
        let rendered = {
            let mut owner = owner
                .try_borrow_mut()
                .map_err(|_| NativeCompositorError::ServiceBusy)?;
            owner.with_raster(bounds.width, bounds.height, |sessions, lease| {
                use crate::display::PixelTarget;
                let format = lease.format();
                #[cfg(feature = "native-compositor")]
                let receipt = crate::display::typography::render_scene_configured(
                    lease,
                    scene,
                    |_, command| command.text_role().into(),
                    expression
                        .as_ref()
                        .map_or(100, |e| e.expression.text_scale_percent),
                    |role| super::super::paint::visual_paint(format, role, expression.as_ref()),
                )?;
                #[cfg(not(feature = "native-compositor"))]
                let receipt = crate::display::render_scene(lease, scene)?;
                // The full packed arena is finite; preparation copies at most
                // MAX_COMPOSITOR_PIXELS unchanged pixels in bounded entries.
                let mut prepared = false;
                for _ in
                    0..=(super::super::MAX_COMPOSITOR_PIXELS / conduitos_compositor_core::MAX_CHUNK)
                {
                    match response(
                        sessions
                            .request(
                                session,
                                Request::PrepareUpload {
                                    pixels: conduitos_compositor_core::MAX_CHUNK as u32,
                                },
                            )
                            .map_err(owner_error)?,
                    )? {
                        Response::Prepared(true) => {
                            prepared = true;
                            break;
                        }
                        Response::Prepared(false) => {}
                        _ => return Err(NativeCompositorError::ServiceProtocol),
                    }
                }
                if !prepared {
                    return Err(NativeCompositorError::ServiceProtocol);
                }
                let mut offset = 0u32;
                for chunk in lease.upload_chunks() {
                    match response(
                        sessions
                            .upload_chunk(session, offset, chunk)
                            .map_err(owner_error)?,
                    )? {
                        Response::Unit => {}
                        _ => return Err(NativeCompositorError::ServiceProtocol),
                    }
                    offset = offset
                        .checked_add(chunk.len() as u32)
                        .ok_or(NativeCompositorError::InvalidBounds)?;
                }
                match response(
                    sessions
                        .request(session, Request::CommitUpload)
                        .map_err(owner_error)?,
                )? {
                    Response::Unit => Ok(receipt),
                    _ => Err(NativeCompositorError::ServiceProtocol),
                }
            })
        };
        let display = match rendered {
            Ok(Ok(receipt)) => receipt,
            Ok(Err(error)) => {
                self.fail();
                return Err(error);
            }
            Err(error) => {
                self.fail();
                return Err(error.into());
            }
        };
        let surface = &mut self.surfaces[i];
        surface.upload_sequence = sequence;
        surface.recipe = Some(*scene);
        surface.binding = Some(Binding {
            manifestation: m.manifestation_id.clone(),
            plan: m.plan_id.clone(),
            placement: m.placement_id.clone(),
            front: m.front_subject.clone(),
            face_revision: face.revision,
        });
        surface.receipt = Some(CompositionReceipt {
            face_id: face.identity.clone(),
            manifestation_id: m.manifestation_id.clone(),
            plan_id: m.plan_id.clone(),
            active_play_id: m.active_play_id.clone(),
            play_sequence: m.play_sequence,
            placement_id: m.placement_id.clone(),
            host_id: m.host_id.clone(),
            boot_id: m.boot_id.clone(),
            offer_generation: m.offer_generation,
            presenter_implementation_id: m.presenter_implementation_id.clone(),
            presenter_capability_id: m.presenter_capability_id.clone(),
            presenter_artifact_id: m.presenter_artifact_id.clone(),
            front_subject: m.front_subject.clone(),
            display_base_id: self.admission.display_base_id.clone(),
            surface_id: id.into(),
            display,
            visual_expression: expression,
        });
        surface
            .receipt
            .as_ref()
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)
    }
}
fn manifestation_error(error: conduit_face::ManifestationError) -> NativeCompositorError {
    if error == conduit_face::ManifestationError::StaleIdentity {
        NativeCompositorError::StaleIdentity
    } else {
        NativeCompositorError::ManifestationInvalid
    }
}
