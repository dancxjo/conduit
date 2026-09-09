//! Inspectable Crèche state, with the same action availability as its shared draft.
use super::{Arrival, Error, FrontDoor};
use alloc::{format, vec, vec::Vec};
use conduit_presentation::*;

impl Arrival {
    pub(in crate::front_door) fn presentation(
        &self,
        door: &FrontDoor,
    ) -> Result<Presentation, Error> {
        let view = self.draft.presentation().map_err(|_| Error::Presentation)?;
        let host = format!("host/{}/{}", door.host_id.as_str(), door.boot_id.as_str());
        let mut subjects = vec![PresentationSubject {
            identity: host.clone(),
            role: PresentationRole::Host,
            label: "Crèche".into(),
            accessibility_name: "A Body of your own".into(),
        }];
        let mut properties = vec![
            property(
                &host,
                "naming-tradition",
                PresentationPropertyValue::Identity(self.draft.requested_system().into()),
            ),
            property(
                &host,
                "selected-control",
                PresentationPropertyValue::Count(self.focus as u64),
            ),
        ];
        if !self.draft.friendly_name().is_empty() {
            properties.push(property(
                &host,
                "friendly-name",
                PresentationPropertyValue::Text(self.draft.friendly_name().into()),
            ));
        }
        properties.push(property(
            &host,
            "name-empty",
            PresentationPropertyValue::Flag(self.draft.friendly_name().is_empty()),
        ));
        for (index, choice) in self.draft.choices().iter().enumerate() {
            let id = format!("form/{}", choice.form.checked_form_id.as_str());
            subjects.push(PresentationSubject {
                identity: id.clone(),
                role: PresentationRole::Form,
                label: choice.title.clone(),
                accessibility_name: choice.title.clone(),
            });
            properties.extend([
                property(
                    &id,
                    "source-document-id",
                    PresentationPropertyValue::Identity(
                        choice.form.source_document_id.as_str().into(),
                    ),
                ),
                property(
                    &id,
                    "checked-form-id",
                    PresentationPropertyValue::Identity(
                        choice.form.checked_form_id.as_str().into(),
                    ),
                ),
                property(
                    &id,
                    "selected",
                    PresentationPropertyValue::Flag(choice.selected),
                ),
                property(
                    &id,
                    "choice-index",
                    PresentationPropertyValue::Count(index as u64),
                ),
            ]);
        }
        let mut actions = Vec::new();
        project_actions(&view.root, &host, &mut actions);
        Presentation::new_with_semantics(
            door.revision,
            PresentationBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_form_id: None,
                expanded_form_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: Vec::new(),
            },
            subjects,
            Vec::new(),
            properties,
            vec![PresentationText {
                subject: host.clone(),
                text: self
                    .refusal
                    .clone()
                    .unwrap_or_else(|| "Name your Body and choose its first Forms.".into()),
            }],
            actions,
            vec![PresentationDisclosure {
                subject: host,
                level: PresentationDisclosureLevel::Primary,
            }],
        )
        .map_err(|_| Error::Presentation)
    }
}
fn property(subject: &str, name: &str, value: PresentationPropertyValue) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}

fn project_actions(
    node: &SemanticPresentationNode,
    target: &str,
    output: &mut Vec<PresentationAction>,
) {
    let mut add = |action: &SemanticAction| {
        output.push(PresentationAction {
            identity: action.identity.clone(),
            intent: format!("conduit.intent/{}@1", action.identity),
            target: target.into(),
            label: action.label.clone(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: match &action.availability {
                ActionAvailability::Available => PresentationActionAvailability::Available,
                ActionAvailability::Busy { detail } => {
                    PresentationActionAvailability::Unavailable {
                        reason_code: "creche/busy".into(),
                        explanation: detail.clone(),
                    }
                }
                ActionAvailability::Unavailable { detail } => {
                    PresentationActionAvailability::Unavailable {
                        reason_code: "creche/selection-not-ready".into(),
                        explanation: detail.clone(),
                    }
                }
            },
        })
    };
    match &node.mechanism {
        PresentationMechanism::Action(action) => add(action),
        PresentationMechanism::FormField(field) => add(&field.input_action),
        PresentationMechanism::ChoiceGroup { options, .. } => {
            for option in options {
                add(&option.change_action);
            }
        }
        _ => {}
    }
    for child in &node.children {
        project_actions(child, target, output);
    }
}
