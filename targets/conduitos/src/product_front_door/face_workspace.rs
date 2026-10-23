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
use conduit_face::{Face, FaceInteraction, MaskShow};
use conduit_human::KeyEvent;
mod lifecycle;
mod patchbay;
use lifecycle::resolve_current_action;

pub(super) struct FaceWorkspace {
    mask: NativeFaceMask,
    active: bool,
    selected_show: Option<MaskShow>,
    editor_surface: bool,
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
            selected_show: None,
            editor_surface: false,
            next_observation: 1,
            next_play: 1,
            next_interaction: 1,
        })
    }

    pub(super) const fn active(&self) -> bool {
        self.active
    }

    pub(super) fn editor_surface(&self) -> bool {
        self.editor_surface
    }

    pub(super) fn enter_editor(
        &mut self,
        door: &FrontDoor,
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let receipt = self.enter(door, presenter, display)?;
        self.editor_surface = true;
        Ok(receipt)
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
        self.selected_show = None;
        if self.active {
            self.mask.suspend().map_err(|error| error.as_str())?;
            self.active = false;
            self.editor_surface = false;
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
        // Retire authority before attempting a replacement, including failures.
        self.selected_show = None;
        self.mask.suspend().map_err(|error| error.as_str())?;
        let face = door.face().map_err(|error| error.as_str())?;
        let observation = self.next_observation;
        let play = self.next_play;
        self.next_observation = observation
            .checked_add(1)
            .ok_or("workspace-face-observation-bound")?;
        self.next_play = play.checked_add(1).ok_or("workspace-face-play-bound")?;
        let receipt = self
            .mask
            .present(face, observation, play, display)
            .map_err(|error| error.as_str())?;
        self.selected_show = self.mask.show().cloned();
        let shown = self
            .selected_show
            .as_ref()
            .ok_or("workspace-face-show-absent")?;
        super::native_action_evidence::face(
            &door.face().map_err(|error| error.as_str())?,
            shown,
            observation,
            play,
        )?;
        Ok(receipt)
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
        // This physical on-ramp remains owned by the existing planned keyboard
        // and text/edit gears. The Mask retains no draft or editing keymap.
        if journey.foreground_text_editor_active()
            && self
                .mask
                .scene()
                .is_some_and(|scene| !scene.showing_details() && !scene.showing_diagram())
            && !matches!(event.usage(), 58..=69 | 75 | 78)
        {
            super::workspace_input::accept(event, journey, door)?;
            return self.present_current(door, display).map(Some);
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
        let previous_page = self.mask.scene().map_or(0, |scene| scene.page());
        match self
            .mask
            .key(event, sequence, display)
            .map_err(|error| error.as_str())?
        {
            NativeFaceMaskInput::Unchanged => Ok(None),
            NativeFaceMaskInput::Redrawn(receipt) => {
                super::native_action_evidence::reading(&self.mask, event.usage(), previous_page)?;
                Ok(Some(receipt))
            }
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

    /// Preserve the specialized Home/application scene while admitting its
    /// lifecycle binding through a real rendered and acknowledged Mask Show.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn invoke_from_presenter(
        &mut self,
        action: patchbay_control::PatchbayAction,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        make: &MakeRecord,
    ) -> Result<CompositionReceipt, &'static str> {
        if self.active || !door.observes_current_product(journey) {
            return Err("workspace-face-owner-changed");
        }
        self.enter(door, presenter, display)?;
        let owner_revision = journey.revision();
        let result =
            self.invoke_product_action(action, door, journey, display, identities, offer, make);
        // Restore the specialized scene on admission refusal too. A failed
        // restore is surfaced; neither surface may retain an actionable Show.
        let restored = self.leave(door, presenter, display);
        result?;
        let restored = restored?;
        // Play may activate the foreground editor. Preserve its automatic
        // entry after lifecycle admission, while Home and Details keep their
        // own application surface.
        if journey.revision() != owner_revision
            && (journey.foreground_text_editor_active()
                || journey.foreground_canonical_patchbay_active())
            && !door.home_open()
            && !door.exact_details_open()
        {
            let receipt = self.enter(door, presenter, display)?;
            self.editor_surface = journey.foreground_text_editor_active();
            Ok(receipt)
        } else {
            Ok(restored)
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
        if !self.active || self.selected_show.as_ref() != self.mask.show() {
            return Err("workspace-face-show-not-selected");
        }
        let face = door.face().map_err(|error| error.as_str())?;
        let show = self
            .mask
            .show()
            .cloned()
            .ok_or("workspace-face-show-absent")?;
        show.validate(&face)
            .map_err(|_| "workspace-face-stale-show")?;
        if !door.observes_current_product(journey) {
            return Err("workspace-face-owner-changed");
        }
        let semantic = face
            .actions
            .iter()
            .find(|candidate| candidate.intent == action.face_intent())
            .ok_or("workspace-face-action-unknown")?;
        if let Err(refusal) = face.resolve_action(face.revision, &semantic.identity) {
            let reason = match refusal {
                conduit_face::FaceActionRefusal::Unavailable { .. } => "UnavailableAction",
                conduit_face::FaceActionRefusal::Refused { .. } => "RefusedAction",
                conduit_face::FaceActionRefusal::StaleRevision => "StaleFace",
                conduit_face::FaceActionRefusal::UnknownAction => "UnknownAction",
            };
            let before = journey.projection();
            let after = journey.projection();
            super::native_action_evidence::action(
                &face,
                &show,
                None,
                &semantic.intent,
                &before,
                &after,
                Some(reason),
            )?;
            let explanation = match &semantic.availability {
                conduit_face::FaceActionAvailability::Unavailable { explanation, .. }
                | conduit_face::FaceActionAvailability::Refused { explanation, .. } => {
                    explanation.as_str()
                }
                conduit_face::FaceActionAvailability::Available => reason,
            };
            door.action_refused(explanation)
                .map_err(|error| error.as_str())?;
            self.present_current(door, display)?;
            return self
                .mask
                .show_local_notice(explanation, display)
                .map_err(|error| error.as_str());
        }
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
        if !self.active || self.selected_show.as_ref() != Some(show) {
            return Err("workspace-face-show-not-selected");
        }
        // The Mask Fore consumes its session on submission; keep only the
        // acknowledged Show receipt until final owner admission, then retire it.
        self.selected_show = None;
        if !door.observes_current_product(journey) {
            return Err("workspace-face-owner-changed");
        }
        let face = door.face().map_err(|error| error.as_str())?;
        if patchbay::owns_action(&interaction.action_id) {
            return self.apply_patchbay(
                door,
                journey,
                display,
                identities,
                offer,
                make,
                face,
                show,
                interaction,
            );
        }
        let action = resolve_current_action(&face, show, interaction)?;
        let request = journey
            .next_request(action, interaction.target.clone(), face.revision)
            .map_err(|error| error.as_str())?;
        let before = journey.projection();
        journey
            .apply(request, identities, offer, make.build_id, face.revision)
            .map_err(|error| error.as_str())?;
        let after = journey.projection();
        super::native_action_evidence::action(
            &face,
            show,
            Some(interaction),
            action.face_intent(),
            &before,
            &after,
            None,
        )?;
        door.observe_product(journey)
            .map_err(|error| error.as_str())?;
        self.present_current(door, display)
    }
}

#[cfg(test)]
#[path = "face_workspace_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "face_workspace_continuity_tests.rs"]
mod continuity_tests;

#[cfg(test)]
#[path = "face_workspace_editor_admission_tests.rs"]
mod editor_admission_tests;

mod pointer;

#[cfg(test)]
#[path = "face_workspace_patchbay_tests.rs"]
mod patchbay_tests;
