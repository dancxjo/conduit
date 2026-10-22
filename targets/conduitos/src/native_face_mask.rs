//! Host-owned Face publication through one ordinary planned graphical Mask.
//!
//! The producer forwards the exact semantic Face through its own kernel Fore.
//! The Mask's renderer Host Call remains pending until this compositor writes
//! pixels. Only then does its Show Fore become available for typed input.

use alloc::string::{String, ToString};
use conduit_birth_plot::BirthFaceBasis;
use conduit_core::{BootId, HostBaseId, HostId, OfferGeneration};
use conduit_face::{
    Face, FaceInteraction, FaceRole, MaskInteractionCorrelation, MaskShow, RemoteOwnerMaskRouteSeal,
};
use conduit_human::{KeyEvent, KeyTransition};

use crate::{
    display::PixelTarget,
    mask_control::{Adapter, MaskStage, prepare_stage},
    native_compositor::{CompositionReceipt, CompositorAdmission, InputRoute, NativeCompositor},
    native_face_scene::{FaceFocusRequest, FaceSceneError, FaceSceneInput, NativeFaceScene},
    native_face_snapshot::{FaceSnapshotReceipt, FaceSnapshotRefusal, NativeFaceSnapshotProducer},
    native_mask_play::{NativeMaskInteractionSession, NativeMaskPlayError, PreparedNativeMaskPlay},
    native_surface_provider::NativeSurfaceProvider,
};

#[path = "native_face_mask/input.rs"]
mod input;

#[path = "native_face_mask/preparation.rs"]
mod preparation;

#[path = "native_face_mask/encounter.rs"]
mod encounter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeFaceMaskError {
    Plan,
    Face,
    InputUnavailable,
    Producer(FaceSnapshotRefusal),
    Scene(FaceSceneError),
    Mask(NativeMaskPlayError),
    Compositor(crate::native_compositor::NativeCompositorError),
}

impl NativeFaceMaskError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "native-face-mask-plan-refused",
            Self::Face => "native-face-mask-face-refused",
            Self::InputUnavailable => "native-face-mask-input-unavailable",
            Self::Producer(error) => match error {
                FaceSnapshotRefusal::HostIdentity => "native-face-mask-producer-host-identity",
                FaceSnapshotRefusal::Catalog => "native-face-mask-producer-catalog",
                FaceSnapshotRefusal::Plan => "native-face-mask-producer-plan",
                FaceSnapshotRefusal::Shape => "native-face-mask-producer-shape",
                FaceSnapshotRefusal::Face => "native-face-mask-producer-face",
                FaceSnapshotRefusal::ProducerBasis => "native-face-mask-producer-basis",
                FaceSnapshotRefusal::Capacity => "native-face-mask-producer-capacity",
                FaceSnapshotRefusal::Kernel => "native-face-mask-producer-kernel",
                FaceSnapshotRefusal::UnexpectedEffect => "native-face-mask-producer-effect",
                FaceSnapshotRefusal::ForeInput => "native-face-mask-producer-input",
                FaceSnapshotRefusal::ForeOutput => "native-face-mask-producer-output",
                FaceSnapshotRefusal::WorkBound => "native-face-mask-producer-work-bound",
                FaceSnapshotRefusal::PlaySequence => "native-face-mask-producer-play-sequence",
            },
            Self::Scene(FaceSceneError::StaleFace) => "native-face-mask-stale-show",
            Self::Scene(FaceSceneError::UnknownControl) => "native-face-mask-unknown-control",
            Self::Scene(FaceSceneError::UnavailableAction) => "native-face-mask-action-unavailable",
            Self::Scene(FaceSceneError::UnsupportedInput) => "native-face-mask-input-unsupported",
            Self::Scene(FaceSceneError::InputBound) => "native-face-mask-input-bound",
            Self::Scene(FaceSceneError::IncompleteInput) => "native-face-mask-input-incomplete",
            Self::Scene(_) => "native-face-mask-scene-refused",
            Self::Mask(_) => "native-face-mask-execution-refused",
            Self::Compositor(error) => error.as_str(),
        }
    }
}

pub enum NativeFaceMaskInput {
    Unchanged,
    Redrawn(CompositionReceipt),
    Submitted {
        correlation: MaskInteractionCorrelation,
        next_focus: Option<FaceFocusRequest>,
    },
}

/// One Host preparation, shared across changing Face revisions. A Body is not
/// created or advanced here; the caller retains that lifecycle authority.
pub struct NativeFaceMask {
    producer: NativeFaceSnapshotProducer,
    mask: MaskStage,
    compositor: NativeCompositor,
    display_base_id: HostBaseId,
    surface_id: String,
    scene: Option<NativeFaceScene>,
    session: Option<NativeMaskInteractionSession>,
    publication: Option<FaceSnapshotReceipt>,
    surface_admitted: bool,
    interaction_admitted: bool,
    component_configuration: Option<conduit_face::ConfiguredMaskConfiguration>,
    visual_expression: Option<conduit_face::AdmittedVisualExpression>,
    visual_constraints: conduit_face::VisualUserConstraints,
    component_fallback: Option<String>,
    encounter_pending: bool,
}

impl NativeFaceMask {
    /// Apply actual Host user preferences before the next render; Host capacity
    /// remains fixed independently of the selected Look.
    pub fn set_visual_user_constraints(
        &mut self,
        forced_colors: bool,
        reduced_motion: bool,
        minimum_text_scale_percent: u16,
    ) -> Result<(), NativeFaceMaskError> {
        if !(100..=200).contains(&minimum_text_scale_percent) {
            return Err(NativeFaceMaskError::Face);
        }
        self.visual_constraints = conduit_face::VisualUserConstraints {
            forced_colors,
            reduced_motion,
            minimum_text_scale_percent,
            maximum_supported_text_scale_percent: 200,
        };
        Ok(())
    }
    pub fn component_configuration(&self) -> Option<&conduit_face::ConfiguredMaskConfiguration> {
        self.component_configuration.as_ref()
    }
    pub fn visual_expression(&self) -> Option<&conduit_face::AdmittedVisualExpression> {
        self.visual_expression.as_ref()
    }
    pub fn component_fallback(&self) -> Option<&str> {
        self.component_fallback.as_deref()
    }

    pub fn birth_basis(&self, encounter_id: &str) -> BirthFaceBasis {
        let fragment = &self.producer.plan().fragments[0];
        BirthFaceBasis {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            encounter_id: encounter_id.into(),
            producer_plot: self.producer.plot_identity(),
            producer_plan_id: self.producer.plan().plan_id.clone(),
        }
    }

    pub fn publication(&self) -> Option<&FaceSnapshotReceipt> {
        self.publication.as_ref()
    }

    pub fn show(&self) -> Option<&MaskShow> {
        self.session
            .as_ref()
            .map(NativeMaskInteractionSession::show)
    }

    pub fn scene(&self) -> Option<&NativeFaceScene> {
        self.scene.as_ref()
    }

    pub fn show_local_notice(
        &mut self,
        notice: &str,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, NativeFaceMaskError> {
        self.scene
            .as_mut()
            .ok_or(NativeFaceMaskError::Face)?
            .set_local_notice(notice)
            .map_err(NativeFaceMaskError::Scene)?;
        self.repaint(display)
    }

    /// Restore a Mask-local focus request only after the semantic owner has
    /// accepted an interaction and published the replacement Face/Show.
    pub fn focus_named(
        &mut self,
        request: &FaceFocusRequest,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, NativeFaceMaskError> {
        self.scene
            .as_mut()
            .ok_or(NativeFaceMaskError::Face)?
            .focus_named(request)
            .map_err(NativeFaceMaskError::Scene)?;
        self.repaint(display)
    }

    /// Release the arrival surface before the Body's ordinary presenter takes
    /// the finite display. No pending Show remains after this transition.
    pub fn suspend(&mut self) -> Result<(), NativeFaceMaskError> {
        if let Some(session) = self.session.take() {
            session
                .close_without_input()
                .map_err(NativeFaceMaskError::Mask)?;
        }
        if self.surface_admitted {
            self.compositor
                .remove_surface(&self.surface_id)
                .map_err(NativeFaceMaskError::Compositor)?;
            self.surface_admitted = false;
        }
        self.scene = None;
        self.encounter_pending = false;
        Ok(())
    }

    pub fn route_keyboard(&self) -> Result<bool, NativeFaceMaskError> {
        let show = self.show().ok_or(NativeFaceMaskError::Face)?;
        Ok(matches!(
            self.compositor
                .route_keyboard(&show.show.manifestation_id)
                .map_err(NativeFaceMaskError::Compositor)?,
            InputRoute::Delivered(_)
        ))
    }

    /// Complete publication and graphical realization in one bounded host
    /// frame. A failed scanout leaves no new Show and cancels the pending Play.
    pub fn present(
        &mut self,
        face: Face,
        observation_sequence: u64,
        play_sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, NativeFaceMaskError> {
        self.present_with_input(face, observation_sequence, play_sequence, display, true)
    }

    /// A guest can inspect the owner's exact Face without a return route for
    /// semantic interactions. This changes only Mask affordances, never Face
    /// identity, actions, or source wording.
    pub fn present_read_only(
        &mut self,
        face: Face,
        observation_sequence: u64,
        play_sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, NativeFaceMaskError> {
        self.present_with_input(face, observation_sequence, play_sequence, display, false)
    }

    // Private shared preparation boundary: callers retain exact source/Plan custody.
    // Tests use actual checked Host configuration, not a fabricated config record.
    fn prepare_component_scene(
        configuration: &conduit_face::ConfiguredMaskConfiguration,
        face: &Face,
        width: u16,
        height: u16,
        interaction_admitted: bool,
        constraints: conduit_face::VisualUserConstraints,
    ) -> Result<
        (
            NativeFaceScene,
            conduit_face::AdmittedVisualExpression,
            Option<String>,
        ),
        NativeFaceMaskError,
    > {
        let mut fallback = None;
        let requested = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.expression.canonical_value(),
        )
        .map_err(|_| conduit_face::VisualExpressionRefusal::Encoding)
        .and_then(conduit_face::visual_expression_from_info);
        let expression = conduit_face::admit_visual_expression(requested, constraints)
            .map_err(|_| NativeFaceMaskError::Face)?;
        let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .map_err(|_| NativeFaceMaskError::Plan)?;
        let scene = match conduit_face::prepare_component_recipe(recipe, face) {
            Ok(prepared) => {
                let configured = NativeFaceScene::prepare_configured(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    &prepared,
                    &expression,
                );
                match configured {
                    Err(
                        error @ (FaceSceneError::InvalidExtent
                        | FaceSceneError::DocumentBound
                        | FaceSceneError::SceneBound),
                    ) => {
                        fallback = Some(alloc::format!("ConfiguredLayout({error:?})"));
                        NativeFaceScene::prepare_visual_fallback(
                            face.clone(),
                            width,
                            height,
                            interaction_admitted,
                            &expression,
                        )
                    }
                    result => result,
                }
            }
            Err(error) => {
                fallback = Some(alloc::format!("{error:?}"));
                NativeFaceScene::prepare_visual_fallback(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    &expression,
                )
            }
        }
        .map_err(NativeFaceMaskError::Scene)?;
        Ok((scene, expression, fallback))
    }

    fn present_with_input(
        &mut self,
        face: Face,
        observation_sequence: u64,
        play_sequence: u64,
        display: &mut impl PixelTarget,
        interaction_admitted: bool,
    ) -> Result<CompositionReceipt, NativeFaceMaskError> {
        let format = display.format().validate().map_err(|error| {
            NativeFaceMaskError::Compositor(
                crate::native_compositor::NativeCompositorError::Display(error),
            )
        })?;
        let width = u16::try_from(format.width).map_err(|_| NativeFaceMaskError::Face)?;
        let height = u16::try_from(format.height).map_err(|_| NativeFaceMaskError::Face)?;
        let mut scene = if let Some(configuration) = &self.component_configuration {
            let (scene, expression, fallback) = Self::prepare_component_scene(
                configuration,
                &face,
                width,
                height,
                interaction_admitted,
                self.visual_constraints,
            )?;
            self.visual_expression = Some(expression);
            self.component_fallback = fallback;
            scene
        } else {
            NativeFaceScene::prepare_with_input(face.clone(), width, height, interaction_admitted)
                .map_err(NativeFaceMaskError::Scene)?
        };
        self.restore_pending_encounter(&mut scene)?;
        let frame = scene.frame().map_err(NativeFaceMaskError::Scene)?;
        let front_subject = face
            .subjects
            .iter()
            .find(|subject| subject.role == FaceRole::Host)
            .or_else(|| {
                face.subjects
                    .iter()
                    .find(|subject| subject.role == FaceRole::Body)
            })
            .ok_or(NativeFaceMaskError::Face)?
            .identity
            .to_string();
        if let Some(previous) = self.session.take() {
            previous
                .close_without_input()
                .map_err(NativeFaceMaskError::Mask)?;
        }
        self.scene = None;
        self.publication = None;
        let published = self
            .producer
            .forward(face, observation_sequence, play_sequence)
            .map_err(NativeFaceMaskError::Producer)?;
        let pending = PreparedNativeMaskPlay::prepare(
            &self.mask.planned_mask,
            &published.face,
            play_sequence,
            &front_subject,
            &self.surface_id,
            self.display_base_id.clone(),
        )
        .map_err(NativeFaceMaskError::Mask)?;
        let request = pending.renderer_request();
        let bounds = conduit_face::LayoutRect {
            x: 0,
            y: 0,
            width,
            height,
        };
        if !self.surface_admitted {
            self.compositor
                .reacquire_namespace()
                .map_err(NativeFaceMaskError::Compositor)?;
            self.compositor
                .admit_surface(&self.surface_id, bounds, 0)
                .map_err(NativeFaceMaskError::Compositor)?;
            self.surface_admitted = true;
        } else {
            self.compositor
                .place_surface(&self.surface_id, bounds, 0)
                .map_err(NativeFaceMaskError::Compositor)?;
        }
        let composition = if let Some(expression) = &self.visual_expression {
            self.compositor.update_pending_configured_mask_surface(
                request.face(),
                request.prepared_show(),
                request.display_base_id(),
                &frame.scene,
                expression,
            )
        } else {
            self.compositor.update_pending_mask_surface(
                request.face(),
                request.prepared_show(),
                request.display_base_id(),
                &frame.scene,
            )
        }
        .map_err(NativeFaceMaskError::Compositor)?
        .clone();
        self.compositor
            .compose_frame(display)
            .map_err(NativeFaceMaskError::Compositor)?;
        let session = {
            let acknowledgement = self
                .compositor
                .scanout_acknowledgement(&composition, published.face.revision)
                .ok_or(NativeFaceMaskError::Face)?;
            pending
                .complete_render(&acknowledgement)
                .map_err(NativeFaceMaskError::Mask)?
        }; // Release the shared renderer guard before exclusive focus/scanout.
        self.compositor
            .focus_surface(&self.surface_id)
            .map_err(NativeFaceMaskError::Compositor)?;
        self.compositor
            .compose_frame(display)
            .map_err(NativeFaceMaskError::Compositor)?;
        self.publication = Some(published.receipt);
        self.scene = Some(scene);
        self.session = Some(session);
        self.interaction_admitted = interaction_admitted;
        Ok(composition)
    }

    /// The exact interaction returned by this Mask Plot is still a request to
    /// the caller's semantic owner. It does not mutate Birth or Body state.
    pub fn submit(
        &mut self,
        interaction: FaceInteraction,
    ) -> Result<MaskInteractionCorrelation, NativeFaceMaskError> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        self.compositor
            .validate_active_session()
            .map_err(NativeFaceMaskError::Compositor)?;
        if !self.interaction_admitted {
            return Err(NativeFaceMaskError::InputUnavailable);
        }
        self.session
            .take()
            .ok_or(NativeFaceMaskError::Face)?
            .interact(interaction)
            .map_err(NativeFaceMaskError::Mask)
    }

    fn repaint(
        &mut self,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, NativeFaceMaskError> {
        let show = self.show().ok_or(NativeFaceMaskError::Face)?.clone();
        let scene = self.scene.as_ref().ok_or(NativeFaceMaskError::Face)?;
        let frame = scene.frame().map_err(NativeFaceMaskError::Scene)?;
        let receipt = self
            .compositor
            .repaint_mask_surface(scene.face(), &show, &self.display_base_id, &frame.scene)
            .map_err(NativeFaceMaskError::Compositor)?
            .clone();
        if let Err(error) = self.compositor.compose_frame(display) {
            self.session.take();
            return Err(NativeFaceMaskError::Compositor(error));
        }
        if self
            .compositor
            .scanout_acknowledgement(&receipt, show.face_revision)
            .is_none()
        {
            self.session.take();
            return Err(NativeFaceMaskError::Face);
        }
        Ok(receipt)
    }
}

#[cfg(test)]
#[path = "native_face_mask/creche_tests.rs"]
mod creche_tests;
#[cfg(test)]
#[path = "native_face_mask/tests.rs"]
pub(crate) mod tests;
