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

    fn present_current(
        &mut self,
        door: &FrontDoor,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let face = door
            .creche_face(&self.basis)
            .map_err(|error| error.as_str())?;
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
