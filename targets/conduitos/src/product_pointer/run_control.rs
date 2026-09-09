//! Pointer activation calls the existing Tour executor supplied by composition.
use crate::{
    display::PixelTarget,
    native_compositor::RoutedPointer,
    tour_product::{TourProduct, TourProductError, TourProductUpdate},
    tour_shell::TourShellPresenter,
};
use conduit_presentation::{ApplicationEvent, ApplicationEventKind};

pub trait Execute:
    FnMut(&mut TourProduct, &ApplicationEvent) -> Result<TourProductUpdate, TourProductError>
{
}
impl<T: FnMut(&mut TourProduct, &ApplicationEvent) -> Result<TourProductUpdate, TourProductError>>
    Execute for T
{
}

pub(super) fn handle(
    route: &RoutedPointer,
    pressed: bool,
    tour: &mut TourProduct,
    presenter: &mut TourShellPresenter,
    display: &mut impl PixelTarget,
    execute: &mut impl Execute,
) -> Result<bool, &'static str> {
    if !presenter
        .run_hit(route, tour)
        .map_err(|error| error.as_str())?
    {
        return Ok(false);
    }
    presenter
        .set_pointer_hover(true)
        .map_err(|error| error.as_str())?;
    if pressed {
        let event = ApplicationEvent {
            revision: tour.controller().state().revision,
            action: conduit_tour_model::RUN_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: alloc::vec::Vec::new(),
        };
        match execute(tour, &event) {
            Ok(_) => {
                presenter
                    .present(tour, display)
                    .map_err(|error| error.as_str())?;
                crate::arch::early_write(b"CONDUIT_TOUR_CHECKPOINT run-button-completed\n");
            }
            Err(error) if error.controller_refusal().is_some() => {
                presenter
                    .show_transient(
                        tour,
                        conduit_tour_model::TourTransientKind::Refusal,
                        error.as_str(),
                        display,
                    )
                    .map_err(|error| error.as_str())?;
            }
            Err(error) => return Err(error.as_str()),
        }
    } else {
        presenter
            .compose_affordances(display)
            .map_err(|error| error.as_str())?;
    }
    Ok(true)
}
