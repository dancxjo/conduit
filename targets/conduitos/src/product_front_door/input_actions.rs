//! Admitted product actions forwarded by native presentation input.
use super::ENTER;
use crate::{
    front_door::FrontDoor,
    product_bindings::binding_for_usage,
    product_journey::{JourneyAction, JourneyStatus, ProductJourney},
};
use conduit_presentation::{ApplicationAction, ApplicationView};
use patchbay_application::{
    CHANGE_PRESENTERS_ACTION_ID, EDIT_CURRENT_ACTION_ID, INSPECT_NEXT_ACTION_ID,
};

pub(super) fn action_for(
    usage: u8,
    front_door: &FrontDoor,
    journey: &ProductJourney,
) -> Option<JourneyAction> {
    if usage == 65 {
        if journey.status() == JourneyStatus::QuiescentAwaitingInput
            && journey.projection().workload_capacity_available
        {
            return Some(JourneyAction::AdmitForm);
        }
        if journey.status() == JourneyStatus::Lulled {
            return Some(JourneyAction::Fulfill);
        }
    }
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
        60..=65 | 77 => action(
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

pub(super) fn tour_action(usage: u8) -> Option<&'static str> {
    match usage {
        super::F10 => Some(conduit_tour_model::RUN_ACTION_ID),
        super::F11 => Some(conduit_tour_model::OPEN_PATCHBAY_ACTION_ID),
        60 => Some(conduit_tour_model::NEXT_STAGE_ACTION_ID),
        61 => Some(conduit_tour_model::PREVIOUS_STAGE_ACTION_ID),
        62 => Some(conduit_tour_model::NEXT_CHAPTER_ACTION_ID),
        63 => Some(conduit_tour_model::PREVIOUS_CHAPTER_ACTION_ID),
        _ => None,
    }
}

pub(super) fn resident_application_action(
    usage: u8,
    view: &ApplicationView,
) -> Option<&ApplicationAction> {
    let patchbay_action = match usage {
        super::F10 => Some(INSPECT_NEXT_ACTION_ID),
        super::F11 => Some(EDIT_CURRENT_ACTION_ID),
        super::F1 => Some(CHANGE_PRESENTERS_ACTION_ID),
        _ => None,
    };
    tour_action(usage)
        .and_then(|action_id| view.actions.iter().find(|action| action.id == action_id))
        .or_else(|| {
            patchbay_action
                .and_then(|action_id| view.actions.iter().find(|action| action.id == action_id))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};
    use conduit_presentation::{ApplicationEventKind, ApplicationView};
    #[test]
    fn reserved_shell_keys_are_one_explicit_product_control_vocabulary() {
        assert_eq!(product_control(4), None);
        assert_eq!(product_control(40), None);
        for usage in [41, 43, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 77] {
            let action = admitted_product_action(usage).unwrap();
            assert!(!action.id.is_empty());
            assert_eq!(product_control(usage), Some(action.control));
        }
    }

    #[test]
    fn resident_tour_keys_select_semantic_actions_after_navigation_actions() {
        let actions = vec![
            ApplicationAction {
                id: conduit_tour_model::NEXT_CHAPTER_ACTION_ID.to_string(),
                event: ApplicationEventKind::Activate,
            },
            ApplicationAction {
                id: conduit_tour_model::RUN_ACTION_ID.to_string(),
                event: ApplicationEventKind::Activate,
            },
        ];
        let view = ApplicationView {
            revision: 1,
            nodes: vec![],
            actions,
        };
        assert_eq!(
            resident_application_action(super::super::F10, &view).map(|action| action.id.as_str()),
            Some(conduit_tour_model::RUN_ACTION_ID)
        );
    }

    #[test]
    fn native_tour_function_keys_have_explicit_page_and_exercise_meaning() {
        assert_eq!(
            tour_action(60),
            Some(conduit_tour_model::NEXT_STAGE_ACTION_ID)
        );
        assert_eq!(
            tour_action(61),
            Some(conduit_tour_model::PREVIOUS_STAGE_ACTION_ID)
        );
        assert_eq!(
            tour_action(62),
            Some(conduit_tour_model::NEXT_CHAPTER_ACTION_ID)
        );
        assert_eq!(
            tour_action(63),
            Some(conduit_tour_model::PREVIOUS_CHAPTER_ACTION_ID)
        );
    }

    #[test]
    fn resident_patchbay_keys_select_semantic_actions_after_form_actions() {
        let actions = vec![
            ApplicationAction {
                id: "patchbay.form.0".to_string(),
                event: ApplicationEventKind::Activate,
            },
            ApplicationAction {
                id: CHANGE_PRESENTERS_ACTION_ID.to_string(),
                event: ApplicationEventKind::Activate,
            },
            ApplicationAction {
                id: INSPECT_NEXT_ACTION_ID.to_string(),
                event: ApplicationEventKind::Activate,
            },
            ApplicationAction {
                id: EDIT_CURRENT_ACTION_ID.to_string(),
                event: ApplicationEventKind::Activate,
            },
        ];
        let view = ApplicationView {
            revision: 1,
            nodes: vec![],
            actions,
        };

        for (usage, expected) in [
            (super::super::F10, INSPECT_NEXT_ACTION_ID),
            (super::super::F11, EDIT_CURRENT_ACTION_ID),
            (super::super::F1, CHANGE_PRESENTERS_ACTION_ID),
        ] {
            assert_eq!(
                resident_application_action(usage, &view).map(|action| action.id.as_str()),
                Some(expected)
            );
        }
    }
}
