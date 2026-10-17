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
}

impl NativeFaceMask {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
    ) -> Result<Self, NativeFaceMaskError> {
        Self::prepare_with_owner_route(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_owner_route(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
        route: Option<&RemoteOwnerMaskRouteSeal>,
    ) -> Result<Self, NativeFaceMaskError> {
        let producer = NativeFaceSnapshotProducer::prepare(
            host_id.clone(),
            boot_id.clone(),
            offer_generation,
            build_id,
        )
        .map_err(NativeFaceMaskError::Producer)?;
        let mut component_configuration = None;
        let mask = if let Some(route) = route {
            let offer = crate::product_bases::native_mask_host_advertisement(
                &host_id,
                &boot_id,
                offer_generation.0,
                provider,
            );
            route
                .validate_mask_host_offer(&offer)
                .map_err(|_| NativeFaceMaskError::Plan)?;
            let mut selected = None;
            for look in [
                conduit_face::ComponentMaskLook::Compact,
                conduit_face::ComponentMaskLook::Spacious,
            ] {
                for recipe in [
                    conduit_face::TodoComponentRecipe::Flat,
                    conduit_face::TodoComponentRecipe::Grouped,
                ] {
                    if route.planned_mask.mask.plot_name
                        == conduit_face::todo_component_mask_root(look, recipe)
                    {
                        selected = Some((look, recipe));
                    }
                }
            }
            let local = if let Some((look, recipe)) = selected {
                let (stage, configuration) =
                    conduit_conduitos_mask_offer::prepare_component_stage_from_offer(
                        &offer, look, recipe,
                    )
                    .map_err(|_| NativeFaceMaskError::Plan)?;
                component_configuration = Some(configuration);
                stage
            } else {
                conduit_conduitos_mask_offer::prepare_stage_from_offer(
                    conduit_conduitos_mask_offer::Adapter::Native,
                    &offer,
                )
                .map_err(|_| NativeFaceMaskError::Plan)?
            };
            if route.planned_mask != local.planned_mask {
                return Err(NativeFaceMaskError::Plan);
            }
            MaskStage {
                planned_mask: route.planned_mask.clone(),
            }
        } else {
            prepare_stage(
                Adapter::Native,
                &host_id,
                &boot_id,
                1,
                Some(&provider.entry),
            )
            .map_err(|_| NativeFaceMaskError::Plan)?
        };
        let show = mask.planned_mask.show_placement();
        let compositor = NativeCompositor::admitted(
            CompositorAdmission::new(
                host_id,
                boot_id,
                offer_generation,
                show.implementation_id.clone(),
                display_base_id.clone(),
                alloc::vec![show.placement_id.clone()],
                alloc::vec![surface_id.into()],
            )
            .map_err(NativeFaceMaskError::Compositor)?,
        );
        Ok(Self {
            producer,
            mask,
            compositor,
            display_base_id,
            surface_id: surface_id.into(),
            scene: None,
            session: None,
            publication: None,
            surface_admitted: false,
            interaction_admitted: true,
            component_configuration,
            visual_expression: None,
            visual_constraints: conduit_face::VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
            component_fallback: None,
        })
    }

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
        let fallback;
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
                fallback = None;
                NativeFaceScene::prepare_configured(
                    face.clone(),
                    width,
                    height,
                    interaction_admitted,
                    &prepared,
                    &expression,
                )
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
        let scene = if let Some(configuration) = &self.component_configuration {
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
        let acknowledgement = self
            .compositor
            .scanout_acknowledgement(&composition, published.face.revision)
            .ok_or(NativeFaceMaskError::Face)?;
        let session = pending
            .complete_render(&acknowledgement)
            .map_err(NativeFaceMaskError::Mask)?;
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
        if !self.interaction_admitted {
            return Err(NativeFaceMaskError::InputUnavailable);
        }
        self.session
            .take()
            .ok_or(NativeFaceMaskError::Face)?
            .interact(interaction)
            .map_err(NativeFaceMaskError::Mask)
    }

    /// Key translation and local reading state belong to this Mask. A
    /// submitted semantic action exits through the same admitted Mask Fore.
    pub fn key(
        &mut self,
        event: KeyEvent,
        sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<NativeFaceMaskInput, NativeFaceMaskError> {
        if !self.route_keyboard()? {
            return Err(NativeFaceMaskError::Face);
        }
        let show = self.show().ok_or(NativeFaceMaskError::Face)?.clone();
        let scene = self.scene.as_mut().ok_or(NativeFaceMaskError::Face)?;
        let cleared_notice =
            event.transition() == KeyTransition::Pressed && scene.clear_local_notice();
        let result = scene
            .key(event, &show, sequence)
            .map_err(NativeFaceMaskError::Scene)?;
        if cleared_notice && matches!(result, FaceSceneInput::Unchanged) {
            return self.repaint(display).map(NativeFaceMaskInput::Redrawn);
        }
        self.accept_scene_input(result, display)
    }

    pub fn pointer(
        &mut self,
        x: u32,
        y: u32,
        sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<NativeFaceMaskInput, NativeFaceMaskError> {
        let route = self
            .compositor
            .route_pointer(x, y, true)
            .map_err(NativeFaceMaskError::Compositor)?;
        let InputRoute::Delivered(route) = route else {
            return Ok(NativeFaceMaskInput::Unchanged);
        };
        self.compositor
            .validate_pointer_route(&route)
            .map_err(NativeFaceMaskError::Compositor)?;
        let show = self.show().ok_or(NativeFaceMaskError::Face)?.clone();
        if route.manifestation_id != show.show.manifestation_id {
            return Err(NativeFaceMaskError::Face);
        }
        let result = self
            .scene
            .as_mut()
            .ok_or(NativeFaceMaskError::Face)?
            .pointer(route.local_x as i16, route.local_y as i16, &show, sequence)
            .map_err(NativeFaceMaskError::Scene)?;
        self.accept_scene_input(result, display)
    }

    fn accept_scene_input(
        &mut self,
        result: FaceSceneInput,
        display: &mut impl PixelTarget,
    ) -> Result<NativeFaceMaskInput, NativeFaceMaskError> {
        match result {
            FaceSceneInput::Unchanged => Ok(NativeFaceMaskInput::Unchanged),
            FaceSceneInput::Changed => self.repaint(display).map(NativeFaceMaskInput::Redrawn),
            FaceSceneInput::Submitted {
                interaction,
                next_focus,
            } => self
                .submit(*interaction)
                .map(|correlation| NativeFaceMaskInput::Submitted {
                    correlation,
                    next_focus,
                }),
        }
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
#[path = "native_face_mask/tests.rs"]
mod tests;
