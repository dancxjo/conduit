//! Opt-in post-birth inspection through the same admitted Face → Mask → Show
//! path as Crèche. The resident application retains its own semantic owner;
//! this surface only realizes its current Face and returns typed requests.
use crate::{
    display::PixelTarget,
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::BootIdentities,
    make::MakeRecord,
    native_compositor::CompositionReceipt,
    native_face_mask::{NativeFaceMask, NativeFaceMaskInput},
    native_surface_provider::NativeSurfaceProvider,
    offer::HostOffer,
    product_journey::ProductJourney,
};
use conduit_core::{BootId, HostBaseId, HostId, OfferGeneration};
use conduit_human::KeyEvent;
use conduit_presentation::{FaceInteraction, MaskShow, Presentation};

pub(super) struct FaceWorkspace {
    mask: NativeFaceMask,
    active: bool,
    next_observation: u64,
    next_play: u64,
    next_interaction: u64,
}

impl FaceWorkspace {
    pub(super) fn prepare(
        host_id: HostId,
        boot_id: BootId,
        generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        provider: &NativeSurfaceProvider,
    ) -> Result<Self, &'static str> {
        let mask = NativeFaceMask::prepare(
            host_id,
            boot_id,
            generation,
            build_id,
            display_base_id,
            "conduitos/workspace/face",
            provider,
        )
        .map_err(|error| error.as_str())?;
        Ok(Self {
            mask,
            active: false,
            next_observation: 1,
            next_play: 1,
            next_interaction: 1,
        })
    }

    pub(super) const fn active(&self) -> bool {
        self.active
    }

    pub(super) fn continue_after_birth(&mut self, observation: u64, play: u64) {
        self.next_observation = observation;
        self.next_play = play;
    }

    pub(super) fn route_keyboard(&self) -> Result<bool, &'static str> {
        self.mask.route_keyboard().map_err(|error| error.as_str())
    }

    pub(super) fn enter(
        &mut self,
        door: &FrontDoor,
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        if self.active {
            return self.present_current(door, display);
        }
        presenter.suspend().map_err(|error| error.as_str())?;
        let receipt = match self.present_current(door, display) {
            Ok(receipt) => receipt,
            Err(error) => {
                // A refused publication must not strand the workspace with
                // neither presenter owning the admitted surface.
                self.mask.suspend().map_err(|refusal| refusal.as_str())?;
                presenter
                    .present(door, display)
                    .map_err(|restore| restore.as_str())?;
                return Err(error);
            }
        };
        self.active = true;
        Ok(receipt)
    }

    pub(super) fn leave(
        &mut self,
        door: &FrontDoor,
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        self.relinquish()?;
        presenter
            .present(door, display)
            .map_err(|error| error.as_str())
    }

    pub(super) fn relinquish(&mut self) -> Result<(), &'static str> {
        if self.active {
            self.mask.suspend().map_err(|error| error.as_str())?;
            self.active = false;
        }
        Ok(())
    }

    pub(super) fn refresh(
        &mut self,
        door: &mut FrontDoor,
        journey: &ProductJourney,
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        door.observe_product(journey)
            .map_err(|error| error.as_str())?;
        if self.active {
            self.present_current(door, display)
        } else {
            presenter
                .present(door, display)
                .map_err(|error| error.as_str())
        }
    }

    fn present_current(
        &mut self,
        door: &FrontDoor,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let face = door.presentation().map_err(|error| error.as_str())?;
        let observation = self.next_observation;
        let play = self.next_play;
        self.next_observation = observation
            .checked_add(1)
            .ok_or("workspace-face-observation-bound")?;
        self.next_play = play.checked_add(1).ok_or("workspace-face-play-bound")?;
        self.mask
            .present(face, observation, play, display)
            .map_err(|error| error.as_str())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn accept_key(
        &mut self,
        event: KeyEvent,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        display: &mut impl PixelTarget,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        make: &MakeRecord,
    ) -> Result<Option<CompositionReceipt>, &'static str> {
        if !self.active {
            return Err("workspace-face-inactive");
        }
        let show = self
            .mask
            .show()
            .cloned()
            .ok_or("workspace-face-show-absent")?;
        let sequence = self.next_interaction;
        self.next_interaction = sequence
            .checked_add(1)
            .ok_or("workspace-face-input-bound")?;
        match self
            .mask
            .key(event, sequence, display)
            .map_err(|error| error.as_str())?
        {
            NativeFaceMaskInput::Unchanged => Ok(None),
            NativeFaceMaskInput::Redrawn(receipt) => Ok(Some(receipt)),
            NativeFaceMaskInput::Submitted { correlation, .. } => self
                .apply_interaction(
                    door,
                    journey,
                    display,
                    identities,
                    offer,
                    make,
                    &show,
                    &correlation.interaction,
                )
                .map(Some),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn invoke_product_action(
        &mut self,
        action: patchbay_control::PatchbayAction,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        display: &mut impl PixelTarget,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        make: &MakeRecord,
    ) -> Result<CompositionReceipt, &'static str> {
        let face = door.presentation().map_err(|error| error.as_str())?;
        let show = self
            .mask
            .show()
            .cloned()
            .ok_or("workspace-face-show-absent")?;
        let semantic = face
            .actions
            .iter()
            .find(|candidate| candidate.intent == action.presentation_intent())
            .ok_or("workspace-face-action-unknown")?;
        face.resolve_action(face.revision, &semantic.identity)
            .map_err(|_| "workspace-face-action-unavailable")?;
        let sequence = self.next_interaction;
        self.next_interaction = sequence
            .checked_add(1)
            .ok_or("workspace-face-input-bound")?;
        let interaction = FaceInteraction::new(
            &face,
            &show,
            &semantic.identity,
            &semantic.target,
            alloc::vec![],
            sequence,
        )
        .map_err(|_| "workspace-face-interaction-invalid")?;
        let correlation = self
            .mask
            .submit(interaction)
            .map_err(|error| error.as_str())?;
        self.apply_interaction(
            door,
            journey,
            display,
            identities,
            offer,
            make,
            &show,
            &correlation.interaction,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_interaction(
        &mut self,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        display: &mut impl PixelTarget,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        make: &MakeRecord,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<CompositionReceipt, &'static str> {
        let face = door.presentation().map_err(|error| error.as_str())?;
        let action = resolve_current_action(&face, show, interaction)?;
        let request = journey
            .next_request(action, interaction.target.clone(), face.revision)
            .map_err(|error| error.as_str())?;
        journey
            .apply(request, identities, offer, make.build_id, face.revision)
            .map_err(|error| error.as_str())?;
        door.observe_product(journey)
            .map_err(|error| error.as_str())?;
        self.present_current(door, display)
    }
}

fn resolve_current_action(
    face: &Presentation,
    show: &MaskShow,
    interaction: &FaceInteraction,
) -> Result<patchbay_control::PatchbayAction, &'static str> {
    show.validate(face)
        .map_err(|_| "workspace-face-stale-show")?;
    if interaction.face_id != face.identity.as_str()
        || interaction.face_revision != face.revision
        || interaction.show_id != show.show_id.as_str()
        || !interaction.arguments.is_empty()
    {
        return Err("workspace-face-stale-interaction");
    }
    let action = face
        .resolve_action(face.revision, &interaction.action_id)
        .map_err(|_| "workspace-face-action-unavailable")?;
    if action.target != interaction.target {
        return Err("workspace-face-action-target-mismatch");
    }
    let name = action
        .identity
        .strip_prefix("action/")
        .and_then(|remaining| remaining.split_once('/'))
        .filter(|(_, target)| *target == action.target)
        .map(|(name, _)| name)
        .ok_or("workspace-face-action-identity-invalid")?;
    let semantic =
        patchbay_control::PatchbayAction::from_name(name).ok_or("workspace-face-action-unknown")?;
    if semantic.presentation_intent() != action.intent {
        return Err("workspace-face-action-intent-mismatch");
    }
    Ok(semantic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        display::{DisplayError, DisplayFormat},
        native_compositor::InputRoute,
        native_workset::{self, NativePlot},
        product_front_door::workspace_input::tests::listening,
        product_journey::test_support::fixture,
    };
    use conduit_human::{KeyModifiers, KeyTransition};
    use conduit_presentation::GraphicsCommandKind;

    #[derive(Default)]
    struct CountingDisplay(u64);

    impl PixelTarget for CountingDisplay {
        fn format(&self) -> DisplayFormat {
            DisplayFormat {
                width: 640,
                height: 480,
                pitch: 2_560,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }

        fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
            self.0 += 1;
            Ok(())
        }
    }

    #[test]
    fn live_resident_patchbay_face_reaches_mask_show_and_diagram_then_returns_to_application() {
        let (mut journey, mut door) = listening();
        let patchbay = native_workset::resident(NativePlot::Patchbay).unwrap();
        journey.select_plot(&patchbay, journey.revision()).unwrap();
        door.observe_product(&journey).unwrap();
        assert!(journey.foreground_patchbay_graph().is_some());
        let face = door.presentation().unwrap();
        let encoded = serde_json::to_vec(&face).unwrap();
        assert!(
            encoded.len() <= crate::native_face_snapshot::MAX_SNAPSHOT_BYTES,
            "selected Patchbay Face needs {} snapshot bytes",
            encoded.len()
        );
        let projection = journey.projection();
        let provider = crate::product_bases::fixture_surface_provider();
        let mut workspace = FaceWorkspace::prepare(
            projection.host_id.clone(),
            projection.boot_id.clone(),
            projection.offer_generation,
            "build",
            provider.entry.base_id.clone(),
            &provider,
        )
        .unwrap();
        workspace.continue_after_birth(8, 12);
        let mut presenter = FrontDoorPresenter::prepare(
            projection.host_id,
            projection.boot_id,
            projection.offer_generation,
            "profile",
            "image",
            provider.entry.base_id.clone(),
            2,
        )
        .unwrap();
        let mut display = CountingDisplay::default();
        presenter.present(&door, &mut display).unwrap();
        let receipt = workspace
            .enter(&door, &mut presenter, &mut display)
            .unwrap();
        assert_eq!(receipt.presentation_id, face.identity);
        assert_eq!(
            workspace.mask.publication().unwrap().observation_sequence,
            8
        );
        assert_eq!(workspace.mask.show().unwrap().show.play_sequence, 12);
        assert!(workspace.active());
        assert!(workspace.mask.route_keyboard().unwrap());
        assert!(workspace.mask.scene().unwrap().has_diagram());
        let event = KeyEvent::new(60, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
        let (ids, offer, _) = fixture();
        let make = crate::make::MakeRecord {
            build_id: "build",
            ..crate::make::MakeRecord::legacy()
        };
        assert!(
            workspace
                .accept_key(
                    event,
                    &mut door,
                    &mut journey,
                    &mut display,
                    &ids,
                    &offer,
                    &make,
                )
                .unwrap()
                .is_some()
        );
        let scene = workspace.mask.scene().unwrap();
        assert!(scene.showing_diagram());
        assert!(
            scene
                .frame()
                .unwrap()
                .scene
                .commands()
                .iter()
                .any(|command| { command.kind == GraphicsCommandKind::OrthogonalPath })
        );
        let prior_show = workspace.mask.show().unwrap().show_id.clone();
        workspace
            .invoke_product_action(
                patchbay_control::PatchbayAction::Stop,
                &mut door,
                &mut journey,
                &mut display,
                &ids,
                &offer,
                &make,
            )
            .unwrap();
        assert_eq!(
            journey.status(),
            crate::product_journey::JourneyStatus::Stopped
        );
        assert_ne!(workspace.mask.show().unwrap().show_id, prior_show);
        workspace
            .leave(&door, &mut presenter, &mut display)
            .unwrap();
        assert!(!workspace.active());
        assert!(matches!(
            presenter.route_keyboard().unwrap(),
            InputRoute::Delivered(_)
        ));
    }
}
