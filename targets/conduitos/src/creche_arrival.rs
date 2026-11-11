//! Shared Crèche-to-Body lifecycle handoff for ordinary native presenters.
use crate::{
    display::PixelTarget,
    front_door::{FrontDoor, FrontDoorPresenter},
    native_compositor::CompositionReceipt,
    native_workset,
    product_journey::ProductJourney,
};
use conduit_birth_plot::BirthSelection;
pub(crate) fn birth_and_arrive(
    selection: BirthSelection,
    door: &mut FrontDoor,
    journey: &mut ProductJourney,
    presenter: &mut FrontDoorPresenter,
    display: &mut impl PixelTarget,
) -> Result<CompositionReceipt, &'static str> {
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
    presenter.present(door, display).map_err(|e| e.as_str())
}
