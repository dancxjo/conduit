//! Default post-Birth policy over the existing Show-bound lifecycle entrance.
//! Pure Birth remains a separate, lulled lifecycle operation.
use super::*;
use crate::offer::HostOffer;
use patchbay_control::PatchbayAction;

#[allow(clippy::too_many_arguments)]
pub(super) fn after_birth(
    arrival: &FaceArrival,
    door: &mut FrontDoor,
    journey: &mut ProductJourney,
    presenter: &mut FrontDoorPresenter,
    workspace: &mut FaceWorkspace,
    display: &mut impl crate::display::PixelTarget,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    make: &MakeRecord,
) -> Result<(), &'static str> {
    let (observation, play) = arrival.next_publication_sequences();
    workspace.continue_after_birth(observation, play);
    start(
        door, journey, presenter, workspace, display, identities, offer, make,
    )
}

#[allow(clippy::too_many_arguments)]
fn start(
    door: &mut FrontDoor,
    journey: &mut ProductJourney,
    presenter: &mut FrontDoorPresenter,
    workspace: &mut FaceWorkspace,
    display: &mut impl crate::display::PixelTarget,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    make: &MakeRecord,
) -> Result<(), &'static str> {
    if journey.status() != JourneyStatus::BornLulled {
        return Err("post-birth-startup-wrong-state");
    }
    for (action, expected) in [
        (PatchbayAction::Wake, JourneyStatus::Awake),
        (PatchbayAction::Plan, JourneyStatus::Planned),
        (PatchbayAction::Play, JourneyStatus::QuiescentAwaitingInput),
    ] {
        journey_sign::write_observation(
            format!("CONDUIT_STARTUP_STAGE {} begin\n", action.as_str()).as_bytes(),
        );
        // Reuse the current Face, checked Mask Fore, acknowledged Show and
        // canonical owner request. No direct kernel or synthetic action path.
        let outcome = workspace.invoke_from_presenter(
            action, door, journey, presenter, display, identities, offer, make,
        );
        let outcome = match outcome {
            Ok(receipt) if journey.status() == expected => {
                emit_journey_sign(&journey.projection(), make, &receipt);
                workspace_view_sign::emit(door, journey, &receipt)?;
                journey_sign::write_observation(
                    format!("CONDUIT_STARTUP_STAGE {} accepted\n", action.as_str()).as_bytes(),
                );
                continue;
            }
            Ok(_) => "startup-action-unavailable",
            Err(reason) => reason,
        };
        // Birth already succeeded. Keep its Body and all successful lifecycle
        // evidence, stop this finite sequence, and expose the failing stage.
        door.observe_product(journey)
            .map_err(|error| error.as_str())?;
        let reason = format!("{}: {outcome}", action.as_str());
        door.startup_refused(&reason)
            .map_err(|error| error.as_str())?;
        let receipt =
            workspace_input::refresh_with_face(door, journey, presenter, workspace, display)?;
        emit_journey_sign(&journey.projection(), make, &receipt);
        journey_sign::write_observation(
            format!(
                "CONDUIT_STARTUP_STAGE {} refused {outcome}\n",
                action.as_str()
            )
            .as_bytes(),
        );
        return Ok(());
    }
    journey_sign::write_observation(b"CONDUIT_CRECHE_CHECKPOINT body-playing\n");
    Ok(())
}

#[cfg(test)]
mod tests;
