//! Native workspace selection and input failure projection; no execution truth.
use alloc::format;
use conduit_human::{KeyEvent, KeyTransition};

use crate::{
    arch,
    front_door::FrontDoor,
    product_journey::{JourneyError, JourneyStatus, ProductJourney},
};

/// Coalesce observation and pixel work independently at the existing Service
/// boundary. An accepted release advances journey evidence, not visible text.
#[derive(Default)]
pub(super) struct PendingInput {
    changed: bool,
    repaint: bool,
}

impl PendingInput {
    pub(super) fn accept(
        &mut self,
        event: KeyEvent,
        journey: &mut ProductJourney,
        door: &mut FrontDoor,
    ) -> Result<(), &'static str> {
        let revision = journey.revision();
        self.repaint |= accept(event, journey, door)?;
        self.changed |= journey.revision() != revision;
        Ok(())
    }

    pub(super) fn service(
        &mut self,
        door: &mut FrontDoor,
        journey: &ProductJourney,
        presenter: &mut crate::front_door::FrontDoorPresenter,
        display: &mut impl crate::display::PixelTarget,
        visible: bool,
    ) -> Result<Option<crate::native_compositor::CompositionReceipt>, &'static str> {
        let pending = core::mem::take(self);
        if !pending.changed || !visible {
            return Ok(None);
        }
        door.observe_product(journey)
            .map_err(|error| error.as_str())?;
        // Stage an exact retained presentation for metadata-only changes too.
        // Its receipt is observation evidence; only compose writes scanout.
        let receipt = presenter
            .stage(door, display)
            .map_err(|error| error.as_str())?;
        if pending.repaint {
            presenter
                .compose(display)
                .map_err(|error| error.as_str())?;
        }
        Ok(Some(receipt))
    }
}

/// Tab changes foreground membership without replacing the admitted Play.
/// Release is consumed too, so it cannot become an unmatched Form input.
pub(super) fn select(
    event: KeyEvent,
    journey: &mut ProductJourney,
) -> Result<Option<bool>, &'static str> {
    if event.usage() != 43 || journey.workspace_projection().is_none() {
        return Ok(None);
    }
    if event.transition() == KeyTransition::Pressed {
        journey
            .select_next_form(journey.revision())
            .map_err(|error| error.as_str())?;
    }
    Ok(Some(event.transition() == KeyTransition::Pressed))
}

/// Preserve the stopped Body and surface an expected admitted-input refusal.
/// Updating the retained projection does not repaint a foreground Tour surface.
pub(super) fn accept(
    event: KeyEvent,
    journey: &mut ProductJourney,
    door: &mut FrontDoor,
) -> Result<bool, &'static str> {
    update(event, journey, door, |reason| {
        arch::early_write(format!("CONDUIT_WORKSPACE_REFUSAL {reason}\n").as_bytes());
    })
}

fn update(
    event: KeyEvent,
    journey: &mut ProductJourney,
    door: &mut FrontDoor,
    report_refusal: impl FnOnce(&str),
) -> Result<bool, &'static str> {
    let previous = journey.foreground_presentation_sequence();
    match journey.accept_play_input(event) {
        Ok(false) => Ok(false),
        Ok(true) => {
            door.observe_product(journey)
                .map_err(|error| error.as_str())?;
            Ok(journey.foreground_presentation_sequence() != previous)
        }
        Err(error @ JourneyError::Play(_)) if journey.status() == JourneyStatus::Stopped => {
            door.observe_product(journey)
                .map_err(|error| error.as_str())?;
            door.play_refused(error.as_str())
                .map_err(|error| error.as_str())?;
            report_refusal(error.as_str());
            Ok(true)
        }
        Err(error) => Err(error.as_str()),
    }
}

pub(super) fn refresh(
    front_door: &mut FrontDoor,
    journey: &ProductJourney,
    presenter: &mut crate::front_door::FrontDoorPresenter,
    display: &mut impl crate::display::PixelTarget,
) -> Result<crate::native_compositor::CompositionReceipt, &'static str> {
    front_door
        .observe_product(journey)
        .map_err(|error| error.as_str())?;
    presenter
        .present(front_door, display)
        .map_err(|error| error.as_str())
}

#[cfg(test)]
mod service_tests;
#[cfg(test)]
mod tests;
