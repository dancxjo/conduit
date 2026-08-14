//! Canonical semantic-action projection used before native gesture dispatch.

use crate::PatchbayApplication;
use conduit_presentation::{
    Presentation, PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationRole, PresentationSubject,
};
use patchbay_model::PatchbayAction;

const ACTIONS: [PatchbayAction; 10] = [
    PatchbayAction::OpenBack,
    PatchbayAction::Save,
    PatchbayAction::ToggleLinearView,
    PatchbayAction::BeBorn,
    PatchbayAction::Wake,
    PatchbayAction::Plan,
    PatchbayAction::Play,
    PatchbayAction::Hold,
    PatchbayAction::Stop,
    PatchbayAction::Lull,
];

impl PatchbayApplication {
    pub(super) fn semantic_invocation_presentation(&self) -> Result<Presentation, String> {
        if let Some(execution) = &self.renderer_execution {
            return Ok(execution.presentation.clone());
        }
        let graph = self
            .graphical_form
            .as_ref()
            .ok_or("current checked Form projection is absent")?;
        let target = graph.expanded_form_id.as_str().to_owned();
        let flow = self.lifecycle_flow();
        let actions = ACTIONS
            .into_iter()
            .map(|action| {
                let unavailable = if matches!(
                    action,
                    PatchbayAction::OpenBack
                        | PatchbayAction::Save
                        | PatchbayAction::ToggleLinearView
                ) {
                    None
                } else {
                    self.lifecycle_unavailable_reason(action)
                };
                PresentationAction {
                    identity: format!("action/{}/{target}", action.as_str()),
                    intent: action.presentation_intent().into(),
                    target: target.clone(),
                    label: action.as_str().replace('-', " "),
                    disclosure: PresentationDisclosureLevel::CurrentAction,
                    availability: unavailable.map_or(
                        PresentationActionAvailability::Available,
                        |explanation| PresentationActionAvailability::Unavailable {
                            reason_code: format!(
                                "lifecycle/{}",
                                flow.state_code.to_ascii_lowercase()
                            ),
                            explanation,
                        },
                    ),
                }
            })
            .collect();
        let wake = self.build_birth.wake_value();
        let body = self.build_birth.body();
        let embodied = body.is_some() && wake.is_some();
        Presentation::new_with_semantics(
            self.lifecycle_sequence.max(1),
            PresentationBasis {
                seed_id: embodied.then(|| body.unwrap().seed_id.clone()),
                body_id: embodied.then(|| body.unwrap().body_id.clone()),
                wake_id: embodied.then(|| wake.unwrap().wake_id.clone()),
                source_document_id: embodied.then(|| graph.source_document_id.clone()),
                checked_form_id: embodied.then(|| graph.checked_form_id.clone()),
                expanded_form_id: embodied.then(|| graph.expanded_form_id.clone()),
                plan_id: embodied
                    .then(|| self.control.plan().map(|plan| plan.plan_id.clone()))
                    .flatten(),
                active_play_id: embodied
                    .then(|| {
                        wake.and_then(|wake| wake.plans.last())
                            .and_then(|plan| plan.active_play_id.clone())
                    })
                    .flatten(),
                sign_ids: vec![],
            },
            vec![PresentationSubject {
                identity: target.clone(),
                role: PresentationRole::Form,
                label: self
                    .form_editor
                    .as_ref()
                    .map_or_else(|| "Current Form".into(), |editor| editor.view().open_form),
                accessibility_name: "Current checked and expanded Form".into(),
            }],
            vec![],
            vec![],
            vec![],
            actions,
            vec![PresentationDisclosure {
                subject: target,
                level: PresentationDisclosureLevel::Primary,
            }],
        )
        .map_err(|error| format!("semantic invocation Presentation: {error:?}"))
    }
}
