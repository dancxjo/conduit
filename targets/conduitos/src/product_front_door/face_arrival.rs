//! Zero-Body Crèche encounter through the same ordinary Face/Mask boundary.
use crate::{
    arch,
    display::PixelTarget,
    front_door::{FrontDoor, FrontDoorPresenter},
    make::MakeRecord,
    native_compositor::CompositionReceipt,
    native_face_mask::{NativeFaceMask, NativeFaceMaskInput},
    native_face_scene::FaceFocusRequest,
    native_surface_provider::NativeSurfaceProvider,
    product_journey::ProductJourney,
};
use conduit_birth_plot::{BirthActionOutcome, BirthFaceBasis};
use conduit_core::{BootId, HostBaseId, HostId, OfferGeneration};
use conduit_human::KeyEvent;
use conduit_presentation::Presentation;

pub(super) struct FaceArrival {
    mask: NativeFaceMask,
    basis: BirthFaceBasis,
    next_observation: u64,
    next_play: u64,
    next_interaction: u64,
}

pub(super) enum FaceArrivalInput {
    Continue,
    Born,
}

impl FaceArrival {
    /// The post-birth Mask uses the same planned relay and Mask identities.
    /// Continue their publication sequences so active Play IDs do not repeat.
    pub(super) const fn next_publication_sequences(&self) -> (u64, u64) {
        (self.next_observation, self.next_play)
    }

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
            "conduitos/creche/face",
            provider,
        )
        .map_err(|error| error.as_str())?;
        let basis = mask.birth_basis("conduitos/creche/current");
        Ok(Self {
            mask,
            basis,
            next_observation: 1,
            next_play: 1,
            next_interaction: 1,
        })
    }

    pub(super) fn route_keyboard(&self) -> Result<bool, &'static str> {
        self.mask.route_keyboard().map_err(|error| error.as_str())
    }

    pub(super) fn present_first(
        &mut self,
        door: &FrontDoor,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        self.present_current(door, display)?;
        self.mask
            .focus_named(
                &FaceFocusRequest {
                    action_id: "creche.name".into(),
                    argument_name: Some("value".into()),
                },
                display,
            )
            .map_err(|error| error.as_str())
    }

    pub(super) fn present_pending_join(
        &mut self,
        door: &FrontDoor,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let face = door
            .joining_face(&self.basis)
            .map_err(|error| error.as_str())?;
        self.present(face, display)
    }

    pub(super) fn present_owner_face(
        &mut self,
        face: Presentation,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let face_id: alloc::string::String = face.identity.as_str().into();
        let revision = face.revision;
        let observation = self.next_observation;
        let play = self.next_play;
        self.next_observation = observation
            .checked_add(1)
            .ok_or("owner-face-observation-bound")?;
        self.next_play = play.checked_add(1).ok_or("owner-face-play-bound")?;
        let receipt = self
            .mask
            .present_read_only(face, observation, play, display)
            .map_err(|error| error.as_str())?;
        let show = self.mask.show().ok_or("owner-face-show-absent")?;
        let evidence = serde_json::to_vec(&serde_json::json!({
            "schema": "conduit.conduitos/native-owner-face@1",
            "status": "shown",
            "face_id": face_id,
            "face_revision": revision,
            "show_id": show.show_id.as_str(),
            "show_acknowledged": true,
            "interactions_admitted": false,
            "continuing_owner_route": false,
        }))
        .map_err(|_| "owner-face-evidence-encoding-invalid")?;
        if evidence.len() > 1_024 {
            return Err("owner-face-evidence-bound-exceeded");
        }
        arch::early_write(b"CONDUIT_NATIVE_OWNER_FACE ");
        arch::early_write(&evidence);
        arch::early_write(b"\n");
        Ok(receipt)
    }

    /// A one-shot snapshot permits only Mask-local reading after scanout.
    /// It has no admitted path for a semantic action to reach the owner.
    pub(super) fn accept_guest_key(
        &mut self,
        event: KeyEvent,
        display: &mut impl PixelTarget,
    ) -> Result<(), &'static str> {
        let sequence = self.next_interaction;
        self.next_interaction = sequence.checked_add(1).ok_or("owner-face-input-bound")?;
        match self
            .mask
            .key(event, sequence, display)
            .map_err(|error| error.as_str())?
        {
            NativeFaceMaskInput::Unchanged | NativeFaceMaskInput::Redrawn(_) => Ok(()),
            NativeFaceMaskInput::Submitted { .. } => Err("owner-face-action-route-unadmitted"),
        }
    }

    fn present_current(
        &mut self,
        door: &FrontDoor,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let face = door
            .creche_face(&self.basis)
            .map_err(|error| error.as_str())?;
        self.present(face, display)
    }

    fn present(
        &mut self,
        face: conduit_presentation::Presentation,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let observation = self.next_observation;
        let play = self.next_play;
        self.next_observation = observation
            .checked_add(1)
            .ok_or("creche-face-observation-bound")?;
        self.next_play = play.checked_add(1).ok_or("creche-face-play-bound")?;
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
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
        make: &MakeRecord,
    ) -> Result<FaceArrivalInput, &'static str> {
        let show = self.mask.show().cloned().ok_or("creche-face-show-absent")?;
        let sequence = self.next_interaction;
        self.next_interaction = sequence.checked_add(1).ok_or("creche-face-input-bound")?;
        match self
            .mask
            .key(event, sequence, display)
            .map_err(|error| error.as_str())?
        {
            NativeFaceMaskInput::Unchanged => Ok(FaceArrivalInput::Continue),
            NativeFaceMaskInput::Redrawn(_) => {
                arch::early_write(b"CONDUIT_CRECHE_CHECKPOINT edited\n");
                Ok(FaceArrivalInput::Continue)
            }
            NativeFaceMaskInput::Submitted {
                correlation,
                next_focus,
            } => {
                match door
                    .apply_creche_face_interaction(&self.basis, &show, &correlation.interaction)
                    .map_err(|error| error.as_str())?
                {
                    BirthActionOutcome::Changed => {
                        self.present_current(door, display)?;
                        let focus = next_focus.unwrap_or_else(|| FaceFocusRequest {
                            action_id: correlation.interaction.action_id.clone(),
                            argument_name: correlation
                                .interaction
                                .arguments
                                .first()
                                .map(|argument| argument.name.clone()),
                        });
                        self.mask
                            .focus_named(&focus, display)
                            .map_err(|error| error.as_str())?;
                        arch::early_write(b"CONDUIT_CRECHE_CHECKPOINT edited\n");
                        Ok(FaceArrivalInput::Continue)
                    }
                    BirthActionOutcome::Birth(selection) => {
                        self.mask.suspend().map_err(|error| error.as_str())?;
                        super::arrival::birth_and_arrive(
                            selection, door, journey, presenter, display, make,
                        )?;
                        Ok(FaceArrivalInput::Born)
                    }
                }
            }
        }
    }
}
