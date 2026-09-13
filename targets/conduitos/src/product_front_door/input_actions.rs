//! Foreground ownership and Presenter-local lifecycle key bindings.
use super::ENTER;
use crate::{
    front_door::FrontDoor,
    product_bindings::binding_for_usage,
    product_journey::{JourneyAction, JourneyStatus, ProductJourney},
};

pub(super) fn action_for(
    usage: u8,
    front_door: &FrontDoor,
    journey: &ProductJourney,
) -> Option<JourneyAction> {
    if usage == ENTER && !front_door.exact_details_open() {
        return Some(JourneyAction::OpenBack);
    }
    let action = binding_for_usage(usage)?.action;
    if action == JourneyAction::Stop
        && !matches!(
            journey.status(),
            JourneyStatus::QuiescentAwaitingInput | JourneyStatus::SemanticCompleted
        )
    {
        return None;
    }
    Some(action)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProductControl {
    Escape,
    SelectNextForm,
    SuggestOrDetails,
    Lifecycle,
    Tour,
    TourRun,
    TourPatchbay,
    UsbLine,
}

pub(super) fn product_control(usage: u8) -> Option<ProductControl> {
    Some(match usage {
        41 => ProductControl::Escape,
        43 => ProductControl::SelectNextForm,
        59 => ProductControl::SuggestOrDetails,
        60..=65 => ProductControl::Lifecycle,
        66 => ProductControl::Tour,
        67 => ProductControl::TourRun,
        68 => ProductControl::TourPatchbay,
        69 => ProductControl::UsbLine,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reserved_shell_keys_are_one_explicit_product_control_vocabulary() {
        assert_eq!(product_control(4), None);
        assert_eq!(product_control(40), None);
        for usage in [41, 43, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69] {
            assert!(product_control(usage).is_some());
        }
    }
}
pub(super) fn tour_action(usage: u8) -> Option<&'static str> {
    match usage {
        super::F10 => Some(conduit_tour_model::RUN_ACTION_ID),
        super::F11 => Some(conduit_tour_model::OPEN_PATCHBAY_ACTION_ID),
        _ => None,
    }
}
