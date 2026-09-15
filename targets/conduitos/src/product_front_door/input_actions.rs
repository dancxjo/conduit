//! Admitted product actions forwarded by native presentation input.
use super::ENTER;
use crate::{
    front_door::FrontDoor,
    product_bindings::binding_for_usage,
    product_journey::{JourneyAction, JourneyStatus, ProductJourney},
};
use conduit_presentation::{ApplicationAction, ApplicationView};

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

pub(super) fn tour_action(usage: u8) -> Option<&'static str> {
    match usage {
        super::F10 => Some(conduit_tour_model::RUN_ACTION_ID),
        super::F11 => Some(conduit_tour_model::OPEN_PATCHBAY_ACTION_ID),
        _ => None,
    }
}

pub(super) fn resident_application_action(
    usage: u8,
    view: &ApplicationView,
) -> Option<&ApplicationAction> {
    let patchbay_action = match usage {
        super::F10 => Some(patchbay_application::INSPECT_NEXT_ACTION_ID),
        super::F11 => Some(patchbay_application::EDIT_CURRENT_ACTION_ID),
        super::F1 => Some(patchbay_application::CHANGE_PRESENTERS_ACTION_ID),
        _ => None,
    };
    patchbay_action
        .and_then(|action_id| view.actions.iter().find(|action| action.id == action_id))
        .or_else(|| {
            tour_action(usage)
                .and_then(|action_id| view.actions.iter().find(|action| action.id == action_id))
        })
        .or_else(|| {
            let compatibility_index = match usage {
                super::F10 => 0,
                super::F11 => 1,
                super::F1 => 2,
                _ => return None,
            };
            view.actions.get(compatibility_index)
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
        for usage in [41, 43, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69] {
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
    fn resident_patchbay_keys_select_semantic_actions_after_form_actions() {
        let mut actions = (0..4)
            .map(|index| ApplicationAction {
                id: format!(
                    "{}{}",
                    patchbay_application::SELECT_FORM_ACTION_PREFIX,
                    index
                ),
                event: ApplicationEventKind::Activate,
            })
            .collect::<Vec<_>>();
        actions.extend([
            ApplicationAction {
                id: patchbay_application::INSPECT_NEXT_ACTION_ID.to_string(),
                event: ApplicationEventKind::Activate,
            },
            ApplicationAction {
                id: patchbay_application::EDIT_CURRENT_ACTION_ID.to_string(),
                event: ApplicationEventKind::Activate,
            },
            ApplicationAction {
                id: patchbay_application::CHANGE_PRESENTERS_ACTION_ID.to_string(),
                event: ApplicationEventKind::Activate,
            },
        ]);
        let view = ApplicationView {
            revision: 1,
            nodes: vec![],
            actions,
        };
        for (usage, expected) in [
            (
                super::super::F10,
                patchbay_application::INSPECT_NEXT_ACTION_ID,
            ),
            (
                super::super::F11,
                patchbay_application::EDIT_CURRENT_ACTION_ID,
            ),
            (
                super::super::F1,
                patchbay_application::CHANGE_PRESENTERS_ACTION_ID,
            ),
        ] {
            assert_eq!(
                resident_application_action(usage, &view).map(|action| action.id.as_str()),
                Some(expected)
            );
        }
    }
}
