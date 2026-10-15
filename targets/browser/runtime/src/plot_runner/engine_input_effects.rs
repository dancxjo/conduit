//! Planned browser input effects and their admitted preparation metadata.
use super::*;

pub(super) fn effect(contract: &str) -> Option<BrowserHostEffect> {
    use crate::installed_browser as installed;
    Some(match contract {
        installed::KEY_EVENT_OPERATION => BrowserHostEffect::KeyEvent,
        installed::POINTER_EVENT_OPERATION => BrowserHostEffect::PointerEvent,
        installed::BUTTON_EVENT_OPERATION => BrowserHostEffect::ButtonTransition,
        installed::APPLICATION_EVENT_OPERATION | installed::saved_choice::HOST_CALL => {
            BrowserHostEffect::ApplicationEvent
        }
        installed::tutorial_presenter::REQUEST_OPERATION => {
            BrowserHostEffect::TutorialPresenterRequest
        }
        _ => return None,
    })
}

pub(super) fn saved_choice(
    placement: &conduit_core::PlannedGear,
) -> Result<Option<crate::installed_browser::saved_choice::SavedChoiceConfiguration>, String> {
    if placement.kind_id.as_str() == conduit_semantic_catalog::SAVED_CHOICE_KIND {
        crate::installed_browser::saved_choice::configuration(placement).map(Some)
    } else {
        Ok(None)
    }
}

pub(super) fn keymap_back(
    partitions: &[(&PlanFragment, &LoweredPlanFragment)],
    node: NodeId,
    placement: &conduit_core::PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserBack, String> {
    let connected = partitions
        .iter()
        .flat_map(|(_, part)| part.node_specs.iter())
        .nth(usize::from(node.0))
        .is_some_and(|spec| spec.input_cords[1].is_some());
    crate::installed_browser::keymap::prepare_routed(placement, values, connected)
}
