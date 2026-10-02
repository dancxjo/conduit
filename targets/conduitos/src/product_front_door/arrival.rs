//! Composition of Crèche birth with the existing admitted native lifecycle.
use super::emit_journey_sign;
use crate::{
    arch,
    display::PixelTarget,
    front_door::{FrontDoor, FrontDoorPresenter},
    identity::BootIdentities,
    make::MakeRecord,
    native_workset,
    offer::HostOffer,
    product_journey::ProductJourney,
};
use alloc::format;
use conduit_birth_plot::BirthSelection;

pub(super) fn open(
    door: &mut FrontDoor,
    journey: &mut ProductJourney,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    make: &MakeRecord,
) -> Result<(), &'static str> {
    // Crèche is the zero-body entrance. Do not open a plot merely to make the
    // arrival surface exist: reviewed plots belong to the Crèche inventory and
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
    let refusals = native_workset::inventory().map(|plot| {
        native_workset::review(plot, identities, offer, make.build_id)
            .err()
            .map(|error| error.as_str().into())
    });
    door.open_creche_reviewed(uuid, refusals)
        .map_err(|e| e.as_str())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn birth_and_arrive(
    selection: BirthSelection,
    door: &mut FrontDoor,
    journey: &mut ProductJourney,
    presenter: &mut FrontDoorPresenter,
    display: &mut impl PixelTarget,
    make: &MakeRecord,
) -> Result<(), &'static str> {
    journey
        .birth_from_creche(selection)
        .map_err(|e| e.as_str())?;
    let tutorial =
        native_workset::resident(native_workset::NativePlot::Tour).map_err(|e| e.as_str())?;
    if journey
        .biography()
        .is_some_and(|evidence| evidence.body.workset.contains(&tutorial))
    {
        journey
            .select_plot(&tutorial, journey.revision())
            .map_err(|e| e.as_str())?;
    }
    door.observe_product(journey).map_err(|e| e.as_str())?;
    door.close_creche().map_err(|e| e.as_str())?;
    let receipt = presenter.present(door, display).map_err(|e| e.as_str())?;
    emit_journey_sign(&journey.projection(), make, &receipt);
    super::workspace_view_sign::emit(door, journey, &receipt)?;
    arch::early_write(b"CONDUIT_CRECHE_CHECKPOINT body-born-lulled\n");
    Ok(())
}
