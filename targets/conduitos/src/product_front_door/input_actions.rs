//! Admitted product actions forwarded by native presentation input.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProductActionScope {
    Surface,
    Focus,
    Inspection,
    BodyLifecycle,
    TourApplication,
    LineRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AdmittedProductAction {
    pub id: &'static str,
    pub control: ProductControl,
    pub scope: ProductActionScope,
}

pub(super) fn admitted_product_action(usage: u8) -> Option<AdmittedProductAction> {
    Some(match usage {
        41 => action(
            "product.surface.escape",
            ProductControl::Escape,
            ProductActionScope::Surface,
        ),
        43 => action(
            "product.form.select-next",
            ProductControl::SelectNextForm,
            ProductActionScope::Focus,
        ),
        59 => action(
            "product.inspect.suggest-or-details",
            ProductControl::SuggestOrDetails,
            ProductActionScope::Inspection,
        ),
        60..=65 => action(
            "product.body.lifecycle",
            ProductControl::Lifecycle,
            ProductActionScope::BodyLifecycle,
        ),
        66 => action(
            "product.tour.open",
            ProductControl::Tour,
            ProductActionScope::TourApplication,
        ),
        67 => action(
            "product.tour.run",
            ProductControl::TourRun,
            ProductActionScope::TourApplication,
        ),
        68 => action(
            "product.tour.patchbay",
            ProductControl::TourPatchbay,
            ProductActionScope::TourApplication,
        ),
        69 => action(
            "product.line.request",
            ProductControl::UsbLine,
            ProductActionScope::LineRequest,
        ),
        _ => return None,
    })
}

const fn action(
    id: &'static str,
    control: ProductControl,
    scope: ProductActionScope,
) -> AdmittedProductAction {
    AdmittedProductAction { id, control, scope }
}

pub(super) fn product_control(usage: u8) -> Option<ProductControl> {
    admitted_product_action(usage).map(|action| action.control)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reserved_shell_keys_are_one_explicit_product_control_vocabulary() {
        assert_eq!(product_control(4), None);
        assert_eq!(product_control(40), None);
        for usage in [41, 43, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69] {
            let action = admitted_product_action(usage).unwrap();
            assert!(!action.id.is_empty());
            assert_eq!(product_control(usage), Some(action.control));
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
