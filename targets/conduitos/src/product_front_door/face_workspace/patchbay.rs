//! Return only current offered Patchbay actions to the resident owner Fore.
use super::*;

pub(super) fn owns_action(identity: &str) -> bool {
    identity == patchbay_application::EDIT_CURRENT_ACTION_ID
        || identity == patchbay_application::CHANGE_MASKS_ACTION_ID
        || identity.starts_with(patchbay_application::SELECT_PLOT_ACTION_PREFIX)
}
impl FaceWorkspace {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_patchbay(
        &mut self,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        display: &mut impl PixelTarget,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        make: &MakeRecord,
        face: Face,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<CompositionReceipt, &'static str> {
        let action = face
            .actions
            .iter()
            .find(|action| action.identity == input.action_id)
            .ok_or("workspace-face-action-unknown")?;
        let before = journey.projection();
        let refusal = match journey.accept_patchbay_interaction(face.clone(), show.clone(), input) {
            Ok(refusal) => refusal.map(|refusal| alloc::format!("{refusal:?}")),
            Err(crate::product_journey::JourneyError::Play(
                crate::native_workset::PlayRefusal::InputPressure,
            )) => Some("native-body-input-pressure".into()),
            Err(error) => return Err(error.as_str()),
        };
        door.observe_product(journey)
            .map_err(|error| error.as_str())?;
        if let Some(reason) = &refusal {
            door.action_refused(reason)
                .map_err(|error| error.as_str())?;
        } else {
            use crate::native_workset::NativeApplicationRequest::EditCurrent;
            match journey.take_application_request() {
                Some(EditCurrent(
                    request @ patchbay_application::PatchbayApplicationRequest::ChangeMasks {
                        ..
                    },
                )) => {
                    journey
                        .replan_masks(request, identities, offer, make.build_id)
                        .map_err(|error| error.as_str())?;
                    door.observe_product(journey)
                        .map_err(|error| error.as_str())?;
                }
                Some(EditCurrent(request)) => {
                    super::super::patchbay_edit::open_source_editor(
                        door, journey, request, identities,
                    )?;
                }
                None => {}
                Some(_) => return Err("workspace-patchbay-unexpected-owner-request"),
            }
        }
        super::super::native_action_evidence::action(
            &face,
            show,
            Some(input),
            &action.intent,
            &before,
            &journey.projection(),
            refusal.as_deref(),
        )?;
        self.editor_surface = journey.foreground_text_editor_active();
        self.present_current(door, display)
    }
}
