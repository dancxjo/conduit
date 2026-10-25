//! Dispatch retained application requests through their existing runtime owner.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn accept(
    view: &conduit_face::ApplicationView,
    action: &conduit_face::ApplicationAction,
    journey: &mut ProductJourney,
    front_door: &mut FrontDoor,
    runtime: &mut runtime::RootRuntime,
    identities: &BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
    make: &MakeRecord,
) -> Result<(), &'static str> {
    journey
        .accept_application_event(&tutorial::event(view, action))
        .map_err(|error| error.as_str())?;
    match journey.take_application_request() {
        Some(crate::native_workset::NativeApplicationRequest::Tutorial(_)) => {
            return Err("tutorial-request-outside-current-tutorial");
        }
        Some(crate::native_workset::NativeApplicationRequest::RunTour {
            chapter: 0,
            stage: 2,
        }) => {
            let evidence = runtime.run_tour(
                identities,
                offer,
                make.build_id,
                controller_io::RunTourStage {
                    chapter: 0,
                    stage: 2,
                },
            )?;
            journey
                .complete_tour_run(&evidence)
                .map_err(|error| error.as_str())?;
            arch::early_write(b"CONDUIT_WORKSPACE_CHECKPOINT resident-tour-fanout-ran\n");
        }
        Some(crate::native_workset::NativeApplicationRequest::RunTour { chapter, stage }) => {
            let evidence = runtime.run_tour(
                identities,
                offer,
                make.build_id,
                controller_io::RunTourStage { chapter, stage },
            )?;
            journey
                .complete_tour_run(&evidence)
                .map_err(|error| error.as_str())?;
            arch::early_write(b"CONDUIT_WORKSPACE_CHECKPOINT resident-tour-ran\n");
        }
        Some(crate::native_workset::NativeApplicationRequest::EditCurrent(
            request @ patchbay_application::PatchbayApplicationRequest::ChangeMasks { .. },
        )) => {
            journey
                .replan_masks(request, identities, offer, make.build_id)
                .map_err(|error| error.as_str())?;
            arch::early_write(b"CONDUIT_WORKSPACE_CHECKPOINT patchbay-presenters-replanned\n");
        }
        Some(crate::native_workset::NativeApplicationRequest::EditCurrent(request)) => {
            patchbay_edit::open_source_editor(front_door, journey, request, identities)?;
        }
        Some(crate::native_workset::NativeApplicationRequest::OpenPatchbay) => {
            return Err("resident-patchbay-request-not-routed");
        }
        None => {}
    }
    Ok(())
}
