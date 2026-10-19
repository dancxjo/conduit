//! Dispatch existing Home choices to their current product owners.
use super::{
    FaceWorkspace, FrontDoor, FrontDoorPresenter, ProductJourney, arch, emit_journey_sign, refresh,
    tutorial,
};
use crate::{display::PixelTarget, make::MakeRecord};
use alloc::format;
use conduit_human::KeyEvent;

#[allow(clippy::too_many_arguments)]
pub(super) fn accept(
    event: KeyEvent,
    front_door: &mut FrontDoor,
    journey: &mut ProductJourney,
    presenter: &mut FrontDoorPresenter,
    face_workspace: &mut FaceWorkspace,
    display: &mut impl PixelTarget,
    make: &MakeRecord,
) -> Result<(), &'static str> {
    if !front_door.observes_current_product(journey) {
        return Err("home-owner-changed");
    }
    match front_door
        .accept_home(event, front_door.revision())
        .map_err(|error| error.as_str())?
    {
        crate::front_door::HomeInput::Unchanged => {}
        crate::front_door::HomeInput::Changed => {
            presenter
                .present(front_door, display)
                .map_err(|error| error.as_str())?;
            arch::early_write(
                format!(
                    "CONDUIT_HOME_STATE {} {}\n",
                    front_door
                        .home_view()
                        .map_or("closed", crate::front_door::HomeView::as_str),
                    front_door.home_selection().unwrap_or(0)
                )
                .as_bytes(),
            );
        }
        crate::front_door::HomeInput::OpenTour => {
            tutorial::select(journey, front_door, presenter, display, make)?;
        }
        crate::front_door::HomeInput::OpenPatchbay => {
            let requested = front_door
                .home_installed_plots()
                .iter()
                .find(|plot| {
                    crate::native_workset::resolve(&plot.plot)
                        .is_ok_and(|kind| kind == crate::native_workset::NativePlot::Patchbay)
                })
                .ok_or("home-patchbay-not-in-current-body")?
                .plot
                .clone();
            journey
                .select_plot(&requested, journey.revision())
                .map_err(|error| error.as_str())?;
            front_door.close_home().map_err(|error| error.as_str())?;
            let receipt = refresh(front_door, journey, presenter, face_workspace, display)?;
            emit_journey_sign(&journey.projection(), make, &receipt);
            arch::early_write(b"CONDUIT_HOME_CHECKPOINT patchbay-opened\n");
        }
        crate::front_door::HomeInput::OpenCreche => {
            arch::early_write(b"CONDUIT_HOME_CHECKPOINT creche-unavailable-after-birth\n");
            presenter
                .present(front_door, display)
                .map_err(|error| error.as_str())?;
        }
        crate::front_door::HomeInput::OpenPlot(index) => {
            let requested = front_door
                .home_installed_plots()
                .get(index)
                .ok_or("home-plot-not-in-current-body")?
                .plot
                .clone();
            let requested_name = crate::native_workset::resolve(&requested)
                .map_err(|_| "home-resident-identity-invalid")?
                .name();
            journey
                .select_plot(&requested, journey.revision())
                .map_err(|error| error.as_str())?;
            front_door.close_home().map_err(|error| error.as_str())?;
            let receipt = refresh(front_door, journey, presenter, face_workspace, display)?;
            emit_journey_sign(&journey.projection(), make, &receipt);
            arch::early_write(
                format!("CONDUIT_HOME_CHECKPOINT plot-opened {}\n", requested_name).as_bytes(),
            );
        }
        crate::front_door::HomeInput::RunPlot(index) => {
            let requested = front_door
                .home_installed_plots()
                .get(index)
                .ok_or("home-plot-not-in-current-body")?
                .plot
                .clone();
            let requested_name = crate::native_workset::resolve(&requested)
                .map_err(|_| "home-resident-identity-invalid")?
                .name();
            journey
                .select_plot(&requested, journey.revision())
                .map_err(|error| error.as_str())?;
            front_door.close_home().map_err(|error| error.as_str())?;
            let receipt = refresh(front_door, journey, presenter, face_workspace, display)?;
            emit_journey_sign(&journey.projection(), make, &receipt);
            arch::early_write(
                format!(
                    "CONDUIT_HOME_CHECKPOINT plot-selected execution-unchanged {}\n",
                    requested_name
                )
                .as_bytes(),
            );
        }
    }
    Ok(())
}
