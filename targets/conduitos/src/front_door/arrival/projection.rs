//! Inspectable Crèche state, with the same action availability as its shared draft.
use super::{Arrival, Error, FrontDoor};
use alloc::{format, vec, vec::Vec};
use conduit_birth_plot::BirthWidgetFace;
use conduit_face::*;

impl Arrival {
    pub(in crate::front_door) fn face(&self, door: &FrontDoor) -> Result<Face, Error> {
        let view = self.draft.widget_face().map_err(|_| Error::Face)?;
        let host = format!("host/{}/{}", door.host_id.as_str(), door.boot_id.as_str());
        let mut subjects = vec![FaceSubject {
            identity: host.clone(),
            role: FaceRole::Host,
            name: "Crèche — a body of your own".into(),
        }];
        let mut properties = vec![
            property(
                &host,
                "naming-tradition",
                FacePropertyValue::Identity(self.draft.requested_system().into()),
            ),
            property(
                &host,
                "selected-control",
                FacePropertyValue::Count(self.focus() as u64),
            ),
        ];
        if !self.draft.friendly_name().is_empty() {
            properties.push(property(
                &host,
                "friendly-name",
                FacePropertyValue::Text(self.draft.friendly_name().into()),
            ));
        }
        properties.push(property(
            &host,
            "name-empty",
            FacePropertyValue::Flag(self.draft.friendly_name().is_empty()),
        ));
        for (index, choice) in self.draft.choices().iter().enumerate() {
            let id = format!("plot/{}", choice.plot.checked_plot_id.as_str());
            subjects.push(FaceSubject {
                identity: id.clone(),
                role: FaceRole::Plot,
                name: choice.title.clone(),
            });
            properties.extend([
                property(
                    &id,
                    "source-document-id",
                    FacePropertyValue::Identity(choice.plot.source_document_id.as_str().into()),
                ),
                property(
                    &id,
                    "checked-plot-id",
                    FacePropertyValue::Identity(choice.plot.checked_plot_id.as_str().into()),
                ),
                property(&id, "selected", FacePropertyValue::Flag(choice.selected)),
                property(&id, "choice-index", FacePropertyValue::Count(index as u64)),
            ]);
        }
        let mut actions = Vec::new();
        project_actions(&view.root, &host, &mut actions);
        Face::new_with_semantics(
            door.revision,
            FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: Vec::new(),
            },
            subjects,
            Vec::new(),
            properties,
            vec![FaceText {
                subject: host.clone(),
                text: self
                    .refusal()
                    .map(alloc::string::String::from)
                    .unwrap_or_else(|| "Name your body and choose its installed plots.".into()),
            }],
            actions,
            vec![FaceDisclosure {
                subject: host,
                level: FaceDisclosureLevel::Primary,
            }],
        )
        .map_err(|_| Error::Face)
    }
}
fn property(subject: &str, name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}

fn project_actions(node: &SemanticFaceNode, target: &str, output: &mut Vec<FaceAction>) {
    let mut add = |action: &SemanticAction| {
        output.push(FaceAction {
            identity: action.identity.clone(),
            intent: format!("conduit.intent/{}@1", action.identity),
            target: target.into(),
            name: action.label.clone(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: match &action.availability {
                ActionAvailability::Available => FaceActionAvailability::Available,
                ActionAvailability::Busy { detail } => FaceActionAvailability::Unavailable {
                    reason_code: "creche/busy".into(),
                    explanation: detail.clone(),
                },
                ActionAvailability::Unavailable { detail } => FaceActionAvailability::Unavailable {
                    reason_code: "creche/selection-not-ready".into(),
                    explanation: detail.clone(),
                },
            },
        })
    };
    match &node.mechanism {
        FaceMechanism::Action(action) => add(action),
        FaceMechanism::FormField(field) => add(&field.input_action),
        FaceMechanism::ChoiceGroup { options, .. } => {
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
