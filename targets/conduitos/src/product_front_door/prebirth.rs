//! Switch the zero-Body encounter without replacing its retained Birth draft.
use super::*;
use crate::{display::PixelTarget, native_compositor::CompositionReceipt};

pub(super) fn enter_commons(
    door: &mut FrontDoor,
    arrival: &mut FaceArrival,
    presenter: &mut FrontDoorPresenter,
    display: &mut impl PixelTarget,
) -> Result<CompositionReceipt, &'static str> {
    door.open_home().map_err(|error| error.as_str())?;
    arrival.suspend()?;
    let receipt = presenter
        .present(door, display)
        .map_err(|error| error.as_str())?;
    journey_sign::write_observation(b"CONDUIT_COMMONS_CHECKPOINT zero-body-hub\n");
    Ok(receipt)
}

pub(super) fn enter_creche(
    door: &mut FrontDoor,
    arrival: &mut FaceArrival,
    presenter: &mut FrontDoorPresenter,
    display: &mut impl PixelTarget,
) -> Result<CompositionReceipt, &'static str> {
    if !door.commons_creche_selected() {
        return Err("commons-creche-context-not-selected");
    }
    door.enter_prebirth_creche()
        .map_err(|error| error.as_str())?;
    presenter.suspend().map_err(|error| error.as_str())?;
    let receipt = match arrival.present_first(door, display) {
        Ok(receipt) => receipt,
        Err(reason) => {
            arrival.suspend()?;
            door.open_home().map_err(|error| error.as_str())?;
            door.action_refused(reason)
                .map_err(|error| error.as_str())?;
            return presenter
                .present(door, display)
                .map_err(|error| error.as_str());
        }
    };
    journey_sign::write_observation(b"CONDUIT_COMMONS_CHECKPOINT zero-body-creche\n");
    journey_sign::write_observation(b"CONDUIT_CRECHE_CHECKPOINT ready\n");
    Ok(receipt)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn back(
    event: conduit_human::KeyEvent,
    door: &mut FrontDoor,
    journey: &ProductJourney,
    arrival: &mut FaceArrival,
    presenter: &mut FrontDoorPresenter,
    display: &mut impl PixelTarget,
    make: &MakeRecord,
) -> Result<bool, &'static str> {
    if event.usage() != 41
        || event.transition() != KeyTransition::Pressed
        || journey.projection().body_id.is_some()
        || !door.creche_open()
    {
        return Ok(false);
    }
    let receipt = enter_commons(door, arrival, presenter, display)?;
    emit_journey_sign(&journey.projection(), make, &receipt);
    Ok(true)
}
