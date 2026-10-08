//! Zero-Body Crèche encounter through the same ordinary Face/Mask boundary.
use crate::{
    arch,
    display::PixelTarget,
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::BootIdentities,
    make::MakeRecord,
    native_compositor::CompositionReceipt,
    native_face_mask::{NativeFaceMask, NativeFaceMaskInput},
    native_face_scene::FaceFocusRequest,
    native_owner_return::NativeOwnerReturnRoute,
    native_surface_provider::NativeSurfaceProvider,
    product_journey::ProductJourney,
};
use conduit_birth_plot::{BirthActionOutcome, BirthFaceBasis};
use conduit_core::{BootId, HostBaseId, HostId, OfferGeneration};
use conduit_human::KeyEvent;
use conduit_presentation::{FaceInteraction, MaskShow, Presentation, RemoteOwnerMaskRouteSeal};

pub(super) struct FaceArrival {
    mask: NativeFaceMask,
    basis: BirthFaceBasis,
    next_observation: u64,
    next_play: u64,
    next_interaction: u64,
    owner_face: Option<Presentation>,
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
        owner_route: Option<&RemoteOwnerMaskRouteSeal>,
    ) -> Result<Self, &'static str> {
        let mask = NativeFaceMask::prepare_with_owner_route(
            host_id,
            boot_id,
            generation,
            build_id,
            display_base_id,
            "conduitos/creche/face",
            provider,
            owner_route,
        )
        .map_err(|error| error.as_str())?;
        let basis = mask.birth_basis("conduitos/creche/current");
        Ok(Self {
            mask,
            basis,
            next_observation: 1,
            next_play: 1,
            next_interaction: 1,
            owner_face: None,
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
        interactions_admitted: bool,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        // Arrival and the owner Body have independent Face revision sequences.
        // Retire the local surface once; revisions remain monotonic within the
        // newly admitted owner surface thereafter.
        self.retire_arrival_surface()?;
        let face_id: alloc::string::String = face.identity.as_str().into();
        let revision = face.revision;
        let observation = self.next_observation;
        let play = self.next_play;
        self.next_observation = observation
            .checked_add(1)
            .ok_or("owner-face-observation-bound")?;
        self.next_play = play.checked_add(1).ok_or("owner-face-play-bound")?;
        self.owner_face = Some(face.clone());
        let receipt = if interactions_admitted {
            self.mask.present(face, observation, play, display)
        } else {
            self.mask
                .present_read_only(face, observation, play, display)
        }
        .map_err(|error| error.as_str())?;
        let show = self.mask.show().ok_or("owner-face-show-absent")?;
        let evidence = serde_json::to_vec(&serde_json::json!({
            "schema": "conduit.conduitos/native-owner-face@1",
            "status": "shown",
            "face_id": face_id,
            "face_revision": revision,
            "show_id": show.show_id.as_str(),
            "local_show_available": true,
            "owner_show_acknowledged": false,
            "interactions_admitted": interactions_admitted,
            "continuing_owner_route": interactions_admitted,
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

    /// Mask-local reading stays local. A submitted occurrence leaves through
    /// the typed interaction Fore and must be checked by the installed owner.
    pub(super) fn accept_guest_key(
        &mut self,
        event: KeyEvent,
        display: &mut impl PixelTarget,
    ) -> Result<Option<(MaskShow, FaceInteraction)>, &'static str> {
        let show = self.mask.show().cloned().ok_or("owner-face-show-absent")?;
        let sequence = self.next_interaction;
        self.next_interaction = sequence.checked_add(1).ok_or("owner-face-input-bound")?;
        match self
            .mask
            .key(event, sequence, display)
            .map_err(|error| error.as_str())?
        {
            NativeFaceMaskInput::Unchanged | NativeFaceMaskInput::Redrawn(_) => Ok(None),
            NativeFaceMaskInput::Submitted { correlation, .. } => {
                Ok(Some((show, correlation.interaction)))
            }
        }
    }

    pub(super) fn acknowledge_owner_show(
        &self,
        route: &mut NativeOwnerReturnRoute,
        identities: BootIdentities,
    ) -> Result<(), &'static str> {
        let show = self.mask.show().ok_or("owner-face-show-absent")?;
        route.acknowledge_show(identities, show)?;
        arch::early_write(b"CONDUIT_NATIVE_OWNER_FACE {\"schema\":\"conduit.conduitos/native-owner-face@1\",\"status\":\"acknowledged\",\"show_id\":\"");
        arch::early_write(show.show_id.as_str().as_bytes());
        arch::early_write(b"\"}\n");
        Ok(())
    }

    pub(super) fn retire_owner_route(
        &mut self,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let face = self.owner_face.clone().ok_or("owner-face-absent")?;
        self.retire_owner_surface()?;
        self.present_owner_face(face, false, display)
    }

    /// A refreshed owner Face gets a new Show before any return interaction.
    pub(super) fn activate_refreshed_owner_route(
        &mut self,
        face: Presentation,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        self.retire_owner_surface()?;
        self.present_owner_face(face, true, display)
    }

    pub(super) fn show_owner_standby(
        &mut self,
        display: &mut impl PixelTarget,
    ) -> Result<(), &'static str> {
        self.mask
            .show_local_notice(
                "Choose Native graphics in the owner wardrobe, then press F5",
                display,
            )
            .map_err(|error| error.as_str())?;
        Ok(())
    }

    pub(super) fn show_owner_result(
        &mut self,
        accepted: bool,
        refreshed: bool,
        code: &str,
        display: &mut impl PixelTarget,
    ) -> Result<(), &'static str> {
        let code = if code.len() <= 75 && code.bytes().all(|byte| (32..=126).contains(&byte)) {
            code
        } else {
            "interaction-refused"
        };
        let message = if code == "control-outcome-unknown" {
            "Owner outcome unknown; check current Face".into()
        } else if accepted && refreshed {
            "Owner accepted: current Face refreshed".into()
        } else if accepted {
            "Owner accepted; Face unavailable".into()
        } else {
            alloc::format!("Owner refused: {code}")
        };
        self.mask
            .show_local_notice(&message, display)
            .map_err(|error| error.as_str())?;
        Ok(())
    }

    pub(super) fn show_local_refusal(
        &mut self,
        code: &str,
        display: &mut impl PixelTarget,
    ) -> Result<(), &'static str> {
        let message = alloc::format!("Input refused: {code}");
        self.mask
            .show_local_notice(&message, display)
            .map_err(|error| error.as_str())?;
        Ok(())
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

    fn retire_arrival_surface(&mut self) -> Result<(), &'static str> {
        if self.owner_face.is_none() {
            self.mask.suspend().map_err(|error| error.as_str())?;
        }
        Ok(())
    }

    fn retire_owner_surface(&mut self) -> Result<(), &'static str> {
        self.mask.suspend().map_err(|error| error.as_str())
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

#[cfg(all(test, feature = "native-compositor"))]
mod tests {
    use super::*;
    use alloc::vec;
    use conduit_presentation::{Face, FaceContext, FaceFocus};

    struct Display;

    impl PixelTarget for Display {
        fn format(&self) -> crate::display::DisplayFormat {
            crate::display::DisplayFormat {
                width: 640,
                height: 480,
                pitch: 2560,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }

        fn write_pixel(
            &mut self,
            _: u32,
            _: u32,
            _: u32,
        ) -> Result<(), crate::display::DisplayError> {
            Ok(())
        }
    }

    #[test]
    fn owner_face_retires_arrival_revision_domain_once() {
        let provider = crate::product_bases::fixture_surface_provider();
        let mut arrival = FaceArrival::prepare(
            "host/native".into(),
            "boot/native".into(),
            OfferGeneration(1),
            "build",
            provider.entry.base_id.clone(),
            &provider,
            None,
        )
        .unwrap();
        let body = conduit_body::Body::born(
            "source/shared-clock".into(),
            "checked/shared-clock".into(),
            1,
            "sign/owner-born".into(),
        )
        .unwrap();
        let face_at = |revision| {
            Face::project(
                &body,
                None,
                revision,
                FaceContext::Overview,
                FaceFocus::Body,
                vec![],
            )
            .unwrap()
            .presentation
        };
        let local = face_at(9);
        let owner = face_at(7);
        assert_ne!(local.identity, owner.identity);
        assert_eq!(local.revision, 9);
        assert_eq!(owner.revision, 7);
        let mut display = Display;
        arrival.present(local, &mut display).unwrap();
        let local_show = arrival.mask.show().unwrap().show_id.clone();

        arrival.retire_arrival_surface().unwrap();
        assert!(arrival.mask.show().is_none());
        arrival.owner_face = Some(owner.clone());
        let receipt = arrival
            .mask
            .present(
                owner.clone(),
                arrival.next_observation,
                arrival.next_play,
                &mut display,
            )
            .unwrap();
        assert_eq!(receipt.presentation_id, owner.identity);
        assert_ne!(arrival.mask.show().unwrap().show_id, local_show);
        assert_eq!(arrival.mask.scene().unwrap().presentation(), &owner);

        arrival.retire_arrival_surface().unwrap();
        assert_eq!(
            arrival
                .mask
                .present(
                    owner,
                    arrival.next_observation + 1,
                    arrival.next_play + 1,
                    &mut display
                )
                .map_err(|error| error.as_str()),
            Err("compositor-surface-revision-stale")
        );
    }

    #[test]
    fn route_loss_retires_owner_show_before_read_only_republication() {
        let provider = crate::product_bases::fixture_surface_provider();
        let mut arrival = FaceArrival::prepare(
            "host/native".into(),
            "boot/native".into(),
            OfferGeneration(1),
            "build",
            provider.entry.base_id.clone(),
            &provider,
            None,
        )
        .unwrap();
        let body = conduit_body::Body::born(
            "source/shared-clock".into(),
            "checked/shared-clock".into(),
            1,
            "sign/owner-born".into(),
        )
        .unwrap();
        let face = Face::project(
            &body,
            None,
            7,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
        )
        .unwrap()
        .presentation;
        let mut display = Display;
        arrival
            .mask
            .present(face.clone(), 1, 1, &mut display)
            .unwrap();
        arrival.owner_face = Some(face.clone());
        let active_show = arrival.mask.show().unwrap().show_id.clone();

        arrival.retire_owner_surface().unwrap();
        assert!(arrival.mask.show().is_none());
        let read_only = arrival
            .mask
            .present_read_only(face.clone(), 2, 2, &mut display)
            .unwrap();
        assert_eq!(read_only.presentation_id, face.identity);
        assert_ne!(arrival.mask.show().unwrap().show_id, active_show);
        assert_eq!(arrival.mask.scene().unwrap().presentation(), &face);
        assert_eq!(
            arrival
                .mask
                .present_read_only(face, 3, 3, &mut display)
                .map_err(|error| error.as_str()),
            Err("compositor-surface-revision-stale")
        );
    }

    #[test]
    fn read_only_owner_standby_requires_fresh_show_for_explicit_activation() {
        let provider = crate::product_bases::fixture_surface_provider();
        let mut arrival = FaceArrival::prepare(
            "host/native".into(),
            "boot/native".into(),
            OfferGeneration(1),
            "build",
            provider.entry.base_id.clone(),
            &provider,
            None,
        )
        .unwrap();
        let body = conduit_body::Body::born(
            "source/shared-clock".into(),
            "checked/shared-clock".into(),
            1,
            "sign/owner-born".into(),
        )
        .unwrap();
        let face = Face::project(
            &body,
            None,
            7,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
        )
        .unwrap()
        .presentation;
        let mut display = Display;
        arrival
            .mask
            .present_read_only(face.clone(), 1, 1, &mut display)
            .unwrap();
        arrival.owner_face = Some(face.clone());
        let standby_show = arrival.mask.show().unwrap().show_id.clone();
        assert!(arrival.mask.scene().is_some());

        arrival.retire_owner_surface().unwrap();
        assert!(arrival.mask.show().is_none());
        let activated = arrival
            .mask
            .present(face.clone(), 2, 2, &mut display)
            .unwrap();
        assert_eq!(activated.presentation_id, face.identity);
        assert_ne!(arrival.mask.show().unwrap().show_id, standby_show);
        assert_eq!(arrival.mask.scene().unwrap().presentation(), &face);
    }
}
