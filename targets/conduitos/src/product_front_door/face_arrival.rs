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
use conduit_birth_plot::BirthFaceBasis;
use conduit_core::{BootId, HostBaseId, HostId, OfferGeneration};
use conduit_face::{Face, FaceInteraction, MaskShow, RemoteOwnerMaskRouteSeal};
use conduit_human::KeyEvent;

#[path = "face_arrival/selection.rs"]
mod selection;
pub(super) use selection::OwnerSelectionError;

pub(super) struct FaceArrival {
    mask: NativeFaceMask,
    basis: BirthFaceBasis,
    next_observation: u64,
    next_play: u64,
    next_interaction: u64,
    owner_face: Option<Face>,
    preparation: NativeMaskPreparation,
}

struct NativeMaskPreparation {
    host_id: HostId,
    boot_id: BootId,
    generation: OfferGeneration,
    build_id: alloc::string::String,
    display_base_id: HostBaseId,
    provider: NativeSurfaceProvider,
    minimum_text_scale_percent: u16,
    forced_colors: bool,
    owner_host: Option<conduit_face::RemoteMaskHostBasis>,
    route_plan_id: Option<conduit_core::PlanId>,
    selected: Option<(
        conduit_face::ComponentMaskLook,
        conduit_face::TodoComponentRecipe,
    )>,
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
        let mask = if owner_route.is_some() {
            NativeFaceMask::prepare_with_owner_route(
                host_id.clone(),
                boot_id.clone(),
                generation,
                build_id,
                display_base_id.clone(),
                "conduitos/creche/face",
                provider,
                owner_route,
            )
        } else {
            NativeFaceMask::prepare_creche(
                host_id.clone(),
                boot_id.clone(),
                generation,
                build_id,
                display_base_id.clone(),
                "conduitos/creche/face",
                provider,
                conduit_face::CrecheComponentRecipe::ResponsiveColumns,
            )
        }
        .map_err(|error| error.as_str())?;
        let basis = mask.birth_basis("conduitos/creche/current");
        Ok(Self {
            mask,
            basis,
            next_observation: 1,
            next_play: 1,
            next_interaction: 1,
            owner_face: None,
            preparation: NativeMaskPreparation {
                host_id,
                boot_id,
                generation,
                build_id: build_id.into(),
                display_base_id,
                provider: provider.clone(),
                minimum_text_scale_percent: 100,
                forced_colors: false,
                selected: None,
                owner_host: owner_route.map(|route| route.owner_host.clone()),
                route_plan_id: owner_route.map(|route| route.route_plan_id.clone()),
            },
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
        let receipt = self
            .mask
            .focus_named(
                &FaceFocusRequest {
                    action_id: "creche.name".into(),
                    argument_name: Some("value".into()),
                },
                display,
            )
            .map_err(|error| error.as_str())?;
        self.record_focus()?;
        Ok(receipt)
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
        face: Face,
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
            NativeFaceMaskInput::Unchanged => Ok(None),
            NativeFaceMaskInput::Redrawn(_) => {
                self.record_focus()?;
                Ok(None)
            }
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
        if let (Some(face), Some(configuration), Some(expression)) = (
            &self.owner_face,
            self.mask.component_configuration(),
            self.mask.visual_expression(),
        ) {
            let evidence = serde_json::to_vec(&serde_json::json!({
                "schema":"conduit.conduitos/component-realization@1",
                "face":face,
                "face_revision_decimal":alloc::format!("{}",face.revision),
                "owner_host":self.preparation.owner_host,
                "route_plan_id":self.preparation.route_plan_id,
                "configuration":configuration,
                "expression":expression,
                "fallback":self.mask.component_fallback(),
                "acknowledged_show":show,
                "owner_show_acknowledged":true,
            }))
            .map_err(|_| "native-component-evidence-encoding")?;
            if evidence.len() > 65_536 {
                return Err("native-component-evidence-bound");
            }
            arch::early_write(b"CONDUIT_NATIVE_COMPONENT_REALIZATION ");
            arch::early_write(&evidence);
            arch::early_write(b"\n");
        }
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
        face: Face,
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
        let message = if code == "owner-look-provisional-input" {
            "Finish or cancel your edit before changing appearance".into()
        } else {
            alloc::format!("Input refused: {code}")
        };
        self.mask
            .show_local_notice(&message, display)
            .map_err(|error| error.as_str())?;
        Ok(())
    }

    fn record_focus(&self) -> Result<(), &'static str> {
        let scene = self.mask.scene().ok_or("creche-focus-scene-absent")?;
        let show = self.mask.show().ok_or("creche-focus-show-absent")?;
        let control = scene
            .focused()
            .map(|control| {
                scene
                    .resolve(&scene.face().identity, scene.face().revision, control)
                    .map(|(action, argument)| {
                        (
                            action.identity.as_str(),
                            argument.map(|arg| arg.name.as_str()),
                        )
                    })
                    .map_err(|_| "creche-focus-control-absent")
            })
            .transpose()?;
        super::native_action_evidence::focus(
            scene.face(),
            show,
            control,
            scene.page(),
            scene.pages(),
            scene
                .frame()
                .map_err(|_| "creche-focus-frame-unavailable")?
                .hits
                .iter()
                .find(|hit| Some(hit.control) == scene.focused())
                .map(|hit| hit.bounds),
            scene.bounds(),
        )
    }

    fn present_current(
        &mut self,
        door: &FrontDoor,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let face = crate::native_observation::Progress::result(
            crate::native_observation::Phase::FaceConstruction,
            "creche",
            || door.creche_face(&self.basis),
        )
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
        face: conduit_face::Face,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let observation = self.next_observation;
        let play = self.next_play;
        self.next_observation = observation
            .checked_add(1)
            .ok_or("creche-face-observation-bound")?;
        self.next_play = play.checked_add(1).ok_or("creche-face-play-bound")?;
        let receipt = self
            .mask
            .present(face.clone(), observation, play, display)
            .map_err(|error| error.as_str())?;
        let show = self.mask.show().ok_or("creche-face-show-absent")?;
        super::native_action_evidence::face(&face, show, observation, play)?;
        Ok(receipt)
    }
}

mod input;
#[cfg(all(test, feature = "native-compositor"))]
mod tests;

mod look_selection;

mod owner_selection;
