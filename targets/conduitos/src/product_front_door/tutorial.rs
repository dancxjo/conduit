//! User-selected current Workspace Tutorial; never launches the retired chapter shell.
use super::*;
use crate::offer::HostOffer;

#[allow(clippy::too_many_arguments)]
pub(super) fn activate(
    view: &conduit_presentation::ApplicationView,
    action: &conduit_presentation::ApplicationAction,
    journey: &mut crate::product_journey::ProductJourney,
    door: &mut FrontDoor,
    presenter: &mut crate::front_door::FrontDoorPresenter,
    display: &mut impl crate::display::PixelTarget,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    make: &crate::make::MakeRecord,
) -> Result<(), &'static str> {
    let event = event(view, action);
    let outcome = journey.accept_tutorial_action(&event, identities, offer, make.build_id);
    door.observe_product(journey).map_err(|e| e.as_str())?;
    match outcome {
        Ok(crate::product_journey::TutorialSurface::Lifecycle) => {
            door.inspect_lifecycle().map_err(|e| e.as_str())?
        }
        Ok(crate::product_journey::TutorialSurface::Current) => {}
        Err(error) => {
            door.startup_refused(error.as_str())
                .map_err(|e| e.as_str())?;
            arch::early_write(format!("CONDUIT_WORKSPACE_REFUSAL {}\n", error.as_str()).as_bytes());
        }
    }
    let receipt = presenter.present(door, display).map_err(|e| e.as_str())?;
    emit_journey_sign(&journey.projection(), make, &receipt);
    super::workspace_view_sign::emit(door, journey, &receipt)?;
    Ok(())
}

pub(super) fn select(
    journey: &mut crate::product_journey::ProductJourney,
    door: &mut FrontDoor,
    presenter: &mut crate::front_door::FrontDoorPresenter,
    display: &mut impl crate::display::PixelTarget,
    make: &crate::make::MakeRecord,
) -> Result<(), &'static str> {
    let resident = crate::native_workset::resident(crate::native_workset::NativePlot::Tour)
        .map_err(|e| e.as_str())?;
    let selection = journey.select_plot(&resident, journey.revision());
    door.observe_product(journey).map_err(|e| e.as_str())?;
    if let Err(error) = selection {
        door.startup_refused(error.as_str())
            .map_err(|e| e.as_str())?;
    }
    if door.home_open() {
        door.close_home().map_err(|e| e.as_str())?;
    }
    let receipt = presenter.present(door, display).map_err(|e| e.as_str())?;
    emit_journey_sign(&journey.projection(), make, &receipt);
    super::workspace_view_sign::emit(door, journey, &receipt)?;
    Ok(())
}

pub(super) fn navigate(
    event: conduit_human::KeyEvent,
    journey: &crate::product_journey::ProductJourney,
    door: &mut FrontDoor,
    presenter: &mut crate::front_door::FrontDoorPresenter,
    display: &mut impl crate::display::PixelTarget,
) -> Result<bool, &'static str> {
    if event.transition() != KeyTransition::Pressed
        || !door
            .navigate_application(event.usage(), door.revision())
            .map_err(|e| e.as_str())?
    {
        return Ok(false);
    }
    let receipt = presenter.present(door, display).map_err(|e| e.as_str())?;
    super::workspace_view_sign::emit(door, journey, &receipt)?;
    Ok(true)
}

pub(super) fn selected_action(
    usage: u8,
    door: &FrontDoor,
    view: &conduit_presentation::ApplicationView,
) -> Option<conduit_presentation::ApplicationAction> {
    if usage == 40 {
        door.selected_application_action()
    } else {
        resident_application_action(usage, view)
    }
    .cloned()
}

pub(super) fn event(
    view: &conduit_presentation::ApplicationView,
    action: &conduit_presentation::ApplicationAction,
) -> ApplicationEvent {
    ApplicationEvent {
        revision: view.revision,
        action: action.id.clone(),
        kind: action.event,
        value: alloc::vec![],
    }
}
