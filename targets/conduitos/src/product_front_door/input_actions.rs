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
            JourneyStatus::Playing | JourneyStatus::ResultVisible
        )
    {
        return None;
    }
    Some(action)
}

pub(super) fn form_receives_input(tour_open: bool, details_open: bool, usage: u8) -> bool {
    !tour_open
        && !details_open
        && binding_for_usage(usage).is_none()
        && !matches!(usage, 41 | 43 | 59 | 66..=69)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_foreground_form_receives_ordinary_keys() {
        assert!(form_receives_input(false, false, 4));
        assert!(form_receives_input(false, false, 40));
        assert!(!form_receives_input(true, false, 4));
        assert!(!form_receives_input(false, true, 4));
        for usage in [41, 43, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69] {
            assert!(!form_receives_input(false, false, usage));
        }
    }
}
