//! Composition of Crèche birth with the existing admitted native lifecycle.
use super::{emit_journey_sign, refresh};
use crate::{
    arch,
    display::PixelTarget,
    fabrication::FabricationRecord,
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::BootIdentities,
    native_workset,
    offer::HostOffer,
    product_journey::{JourneyAction, ProductJourney},
};
use alloc::format;
use conduit_creche_model::birth::BirthSelection;

pub(super) fn open(
    door: &mut FrontDoor,
    journey: &mut ProductJourney,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    fabrication: &FabricationRecord,
) -> Result<(), &'static str> {
    // Crèche is the zero-Body entrance. Do not open a Form merely to make the
    // arrival surface exist: reviewed Forms belong to the Crèche inventory and
    // become lifecycle truth only after an explicit birth selection.
    door.observe_journey(journey.projection())
        .map_err(|e| e.as_str())?;
    let hex = crate::identity::hex(&identities.boot);
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    let refusals = native_workset::inventory().map(|form| {
        native_workset::review(form, identities, offer, fabrication.build_id)
            .err()
            .map(|error| error.as_str().into())
    });
    door.open_creche_reviewed(uuid, refusals)
        .map_err(|e| e.as_str())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn birth_and_wake(
    selection: BirthSelection,
    door: &mut FrontDoor,
    journey: &mut ProductJourney,
    presenter: &mut FrontDoorPresenter,
    display: &mut impl PixelTarget,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    fabrication: &FabricationRecord,
) -> Result<(), &'static str> {
    journey
        .birth_from_creche(selection)
        .map_err(|e| e.as_str())?;
    door.observe_body(
        journey.projection(),
        journey
            .workspace_projection()
            .ok_or("born-body-workset-absent")?,
    )
    .map_err(|e| e.as_str())?;
    door.close_creche().map_err(|e| e.as_str())?;
    let receipt = presenter.present(door, display).map_err(|e| e.as_str())?;
    emit_journey_sign(&journey.projection(), fabrication, &receipt);
    for action in [
        JourneyAction::Wake,
        JourneyAction::Plan,
        JourneyAction::Play,
    ] {
        let semantic = door
            .resolve_action(action, door.revision())
            .map_err(|e| e.as_str())?;
        let request = journey
            .next_request(action, semantic.target, door.revision())
            .map_err(|e| e.as_str())?;
        if let Err(error) = journey.apply(
            request,
            identities,
            offer,
            fabrication.build_id,
            door.revision(),
        ) {
            door.startup_refused(error.as_str())
                .map_err(|e| e.as_str())?;
            let receipt = refresh(door, journey, presenter, display)?;
            emit_journey_sign(&journey.projection(), fabrication, &receipt);
            arch::early_write(format!("CONDUIT_CRECHE_REFUSAL {}\n", error.as_str()).as_bytes());
            return Ok(());
        }
        let receipt = refresh(door, journey, presenter, display)?;
        emit_journey_sign(&journey.projection(), fabrication, &receipt);
    }
    arch::early_write(b"CONDUIT_CRECHE_CHECKPOINT body-awake\n");
    Ok(())
}
